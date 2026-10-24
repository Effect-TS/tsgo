// Bookkeeping for the Rust port of this repository's Effect Go code (crates/, one crate per Go package).
//
// porting/scope.toml says which Go packages the port covers; porting/manifest.tsv (generated) lists every in-scope
// declaration with a hash of its source; a declaration counts as ported when some Rust file under crates/ carries
// its origin marker on the line above the ported item:
//
//     // go: internal/typeparser/effect_type.go:36 TypeParser.EffectType
//     // go test: internal/typeparser/effect_type_test.go:12 TestEffectType
//
// (`// go test:` for declarations of _test.go files and test-only packages, `// go:` for the rest). A declaration
// deliberately left out is listed with its reason in porting/skipped.tsv. porting/ported.tsv records, per ported
// declaration, the Go hash the Rust port was last synced to (`repoctl tsrs port-ack`), so a later Go change is caught.
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { appendFile } from "node:fs/promises"
import { runCommand, runCommandString } from "./process.ts"

// ---------------------------------------------------------------------------------------------------------------
// Pure model
// ---------------------------------------------------------------------------------------------------------------

export interface Decl {
  readonly path: string
  /** `Func`, `Recv.Method`, type name, first name of a const/var group; `#N` for the Nth repeat in one file. */
  readonly name: string
  readonly kind: string
  readonly line: number
  readonly lines: number
  readonly hash: string
}

/** One declaration as printed by _tools/port_manifest. */
export interface RawDecl {
  readonly file: string
  readonly line: number
  readonly endLine: number
  readonly name: string
  readonly kind: string
  readonly generated: boolean
  readonly hash: string
}

export interface Scope {
  readonly include: ReadonlyArray<string>
  readonly exclude: ReadonlyArray<string>
  readonly excludeFiles: ReadonlyArray<string>
  readonly testPackages: ReadonlyArray<string>
  readonly tests: boolean
  readonly requireNew: boolean
}

export interface Marker {
  readonly path: string
  readonly name: string
  readonly goLine: number
  /** `// go test:` rather than `// go:`. */
  readonly test: boolean
  readonly file: string
  readonly line: number
}

export class PortError extends Data.TaggedError("PortError")<{
  readonly reason: string
}> {
  get message(): string {
    return this.reason
  }
}

export const declId = (path: string, name: string) => `${path} ${name}`

export const packageOf = (path: string) => {
  const index = path.lastIndexOf("/")
  return index === -1 ? "." : path.slice(0, index)
}

export const isTestKind = (kind: string) => kind === "test" || kind === "testhelper"

const compare = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0)

/** The Rust crate of a Go package path ("internal/rules" -> "ets_rules", "etscore" -> "ets_etscore"). */
export const crateFor = (pkg: string) => {
  const normalized = pkg.replace(/\/+$/, "")
  if (normalized === "." || normalized === "") return "ets_root"
  return "ets_" + normalized.replace(/^internal\//, "").toLowerCase().replace(/[^a-z0-9]+/g, "_")
}

// --- scope.toml (the small TOML subset it uses: top-level `key = value`, strings, booleans, string arrays) -------

export const parseScope = (text: string): Scope => {
  const tokens: Array<string | boolean | symbol> = []
  const open = Symbol("[")
  const close = Symbol("]")
  const equals = Symbol("=")
  let i = 0
  while (i < text.length) {
    const c = text[i]!
    if (c === "#") {
      while (i < text.length && text[i] !== "\n") i++
    } else if (/\s|,/.test(c)) {
      i++
    } else if (c === "[") {
      tokens.push(open)
      i++
    } else if (c === "]") {
      tokens.push(close)
      i++
    } else if (c === "=") {
      tokens.push(equals)
      i++
    } else if (c === "\"") {
      let j = i + 1
      while (j < text.length && text[j] !== "\"") j += text[j] === "\\" ? 2 : 1
      tokens.push(JSON.parse(text.slice(i, j + 1)) as string)
      i = j + 1
    } else {
      const m = /^[A-Za-z0-9_-]+/.exec(text.slice(i))
      if (m === null) throw new PortError({ reason: `porting/scope.toml: unexpected ${JSON.stringify(c)}` })
      tokens.push(m[0] === "true" ? true : m[0] === "false" ? false : Symbol.for(m[0]))
      i += m[0].length
    }
  }
  const values = new Map<string, unknown>()
  let t = 0
  while (t < tokens.length) {
    const key = tokens[t]
    if (typeof key !== "symbol" || Symbol.keyFor(key) === undefined || tokens[t + 1] !== equals) {
      throw new PortError({ reason: "porting/scope.toml: expected `key = value` (tables are not supported)" })
    }
    t += 2
    const value = tokens[t]
    if (value === open) {
      const items: Array<string> = []
      t++
      while (tokens[t] !== close) {
        const item = tokens[t]
        if (typeof item !== "string") {
          throw new PortError({ reason: `porting/scope.toml: ${Symbol.keyFor(key)} must be an array of strings` })
        }
        items.push(item)
        t++
      }
      values.set(Symbol.keyFor(key)!, items)
    } else if (typeof value === "string" || typeof value === "boolean") {
      values.set(Symbol.keyFor(key)!, value)
    } else {
      throw new PortError({ reason: `porting/scope.toml: bad value for ${Symbol.keyFor(key)}` })
    }
    t++
  }
  const strings = (key: string) => (values.get(key) ?? []) as ReadonlyArray<string>
  return {
    include: strings("include"),
    exclude: strings("exclude"),
    excludeFiles: strings("exclude_files"),
    testPackages: strings("test_packages"),
    tests: values.get("tests") !== false,
    requireNew: values.get("require_new") === true
  }
}

/** Whether a package directory matches a scope entry ("a/b" exactly, "a/..." for a and everything below). */
export const matchesEntry = (pkg: string, entry: string) =>
  entry.endsWith("/...") ? pkg === entry.slice(0, -4) || pkg.startsWith(entry.slice(0, -3)) : pkg === entry

export type PackageClass = "included" | "excluded" | "unclassified"

export const classifyPackage = (scope: Scope, pkg: string): PackageClass =>
  scope.exclude.some((e) => matchesEntry(pkg, e))
    ? "excluded"
    : scope.include.some((e) => matchesEntry(pkg, e))
    ? "included"
    : "unclassified"

// --- manifest ---------------------------------------------------------------------------------------------------

export const buildManifest = (raw: ReadonlyArray<RawDecl>, scope: Scope): Array<Decl> => {
  const excludedFiles = new Set(scope.excludeFiles)
  const testPackages = new Set(scope.testPackages)
  const seen = new Map<string, number>()
  const decls: Array<Decl> = []
  for (const d of raw) {
    if (d.generated || excludedFiles.has(d.file)) continue
    const kind = testPackages.has(packageOf(d.file)) && !isTestKind(d.kind) ? "testhelper" : d.kind
    if (isTestKind(kind) && !scope.tests) continue
    const key = declId(d.file, d.name)
    const count = (seen.get(key) ?? 0) + 1
    seen.set(key, count)
    decls.push({
      path: d.file,
      name: count === 1 ? d.name : `${d.name}#${count}`,
      kind,
      line: d.line,
      lines: d.endLine - d.line + 1,
      hash: d.hash
    })
  }
  return decls
}

const manifestHeader = "# path\tname\tkind\tline\tlines\thash  (generated by `repoctl tsrs port-manifest --write`; do not edit)"

export const manifestText = (decls: ReadonlyArray<Decl>) =>
  [manifestHeader, ...decls.map((d) => `${d.path}\t${d.name}\t${d.kind}\t${d.line}\t${d.lines}\t${d.hash}`)]
    .join("\n") + "\n"

const dataLines = (text: string) => text.split(/\r?\n/).filter((line) => line.trim() !== "" && !line.startsWith("#"))

export const parseManifest = (text: string): Array<Decl> =>
  dataLines(text).map((line) => {
    const [path, name, kind, ln, lines, hash] = line.split("\t")
    return { path: path!, name: name!, kind: kind!, line: Number(ln), lines: Number(lines), hash: hash! }
  })

/** porting/skipped.tsv: id -> reason. */
export const parseSkipped = (text: string): Map<string, string> => {
  const skipped = new Map<string, string>()
  for (const line of dataLines(text)) {
    const [path, name, ...reason] = line.split("\t")
    skipped.set(declId(path!, name ?? ""), reason.join("\t"))
  }
  return skipped
}

/** porting/ported.tsv: id -> the Go hash the Rust port was last synced to. */
export const parsePorted = (text: string): Map<string, string> => {
  const ported = new Map<string, string>()
  for (const line of dataLines(text)) {
    const [path, name, hash] = line.split("\t")
    ported.set(declId(path!, name ?? ""), hash ?? "")
  }
  return ported
}

const portedHeader = "# path\tname\thash  (Go hash each ported declaration was last synced to; `repoctl tsrs port-ack` updates it)"

export const portedText = (ported: ReadonlyMap<string, string>) => {
  const rows = [...ported].sort(([a], [b]) => compare(a, b)).map(([id, hash]) => {
    const space = id.indexOf(" ")
    return `${id.slice(0, space)}\t${id.slice(space + 1)}\t${hash}`
  })
  return [portedHeader, ...rows].join("\n") + "\n"
}

// --- markers ----------------------------------------------------------------------------------------------------

const markerPattern = /^(\s*\/\/ go( test)?: )(\S+?\.go):(\d+) (\S+)\s*$/

export const parseMarkers = (text: string, file: string): Array<Marker> => {
  const markers: Array<Marker> = []
  text.split("\n").forEach((line, index) => {
    const m = markerPattern.exec(line)
    if (m !== null) {
      markers.push({ path: m[3]!, name: m[5]!, goLine: Number(m[4]), test: m[2] !== undefined, file, line: index + 1 })
    }
  })
  return markers
}

/** Rewrites marker line numbers to the manifest's; markers naming unknown declarations are left alone. */
export const fixMarkerLines = (text: string, manifest: ReadonlyArray<Decl>) => {
  const lines = new Map(manifest.map((d) => [declId(d.path, d.name), d.line]))
  return text.split("\n").map((line) => {
    const m = markerPattern.exec(line)
    if (m === null) return line
    const goLine = lines.get(declId(m[3]!, m[5]!))
    return goLine === undefined || goLine === Number(m[4]) ? line : line.replace(`.go:${m[4]} `, `.go:${goLine} `)
  }).join("\n")
}

const markerText = (d: Decl) => `// go${isTestKind(d.kind) ? " test" : ""}: ${d.path}:${d.line} ${d.name}`

// --- check ------------------------------------------------------------------------------------------------------

export interface CheckInput {
  readonly manifest: ReadonlyArray<Decl>
  readonly markers: ReadonlyArray<Marker>
  readonly skipped: ReadonlyMap<string, string>
  readonly ported: ReadonlyMap<string, string>
  readonly requireNew: boolean
}

export const checkPort = ({ manifest, markers, ported, requireNew, skipped }: CheckInput): Array<string> => {
  const errors: Array<string> = []
  const entries = new Map(manifest.map((d) => [declId(d.path, d.name), d]))
  const marked = new Set<string>()
  for (const m of markers) {
    const id = declId(m.path, m.name)
    const d = entries.get(id)
    const where = `${m.file}:${m.line}`
    if (d === undefined) {
      errors.push(`${where}: marker names '${id}', which is not in porting/manifest.tsv`)
      continue
    }
    marked.add(id)
    if (d.line !== m.goLine) {
      errors.push(`${where}: ${id} is at line ${d.line}, marker says ${m.goLine} (run \`repoctl tsrs port-fix-lines\`)`)
    }
    if (isTestKind(d.kind) !== m.test) {
      errors.push(`${where}: ${id} is ${isTestKind(d.kind) ? "a test" : "not a test"} declaration: write \`${markerText(d)}\``)
    }
  }
  for (const id of skipped.keys()) {
    if (!entries.has(id)) errors.push(`porting/skipped.tsv: '${id}' is not in porting/manifest.tsv`)
    else if (marked.has(id)) errors.push(`porting/skipped.tsv: '${id}' is skipped but also has a marker`)
  }
  for (const id of [...marked].sort(compare)) {
    const hash = ported.get(id)
    const space = id.indexOf(" ")
    const ack = `repoctl tsrs port-ack ${id.slice(0, space)} ${id.slice(space + 1)}`
    if (hash === undefined) {
      errors.push(`porting/ported.tsv: ${id} is marked as ported but not recorded: run \`${ack}\``)
    } else if (hash !== entries.get(id)!.hash) {
      errors.push(`porting/ported.tsv: ${id}: Go declaration changed since it was ported: update the Rust port, then \`${ack}\``)
    }
  }
  for (const id of ported.keys()) {
    if (!marked.has(id)) {
      errors.push(`porting/ported.tsv: '${id}' has no marker under crates/ (run \`repoctl tsrs port-ack --prune\`)`)
    }
  }
  if (requireNew) {
    for (const d of manifest) {
      const id = declId(d.path, d.name)
      if (!marked.has(id) && !skipped.has(id)) {
        errors.push(`${d.path}:${d.line}: ${d.name} is neither ported nor in porting/skipped.tsv (require_new = true)`)
      }
    }
  }
  return errors
}

// --- ack --------------------------------------------------------------------------------------------------------

export interface AckRequest {
  /** Explicit "path name" ids. */
  readonly ids: ReadonlyArray<string>
  /** Ack every marked declaration whose path starts with one of these. */
  readonly prefixes: ReadonlyArray<string>
  /** Drop ported.tsv entries that have no marker. */
  readonly prune: boolean
}

export const ackPorted = (
  ported: ReadonlyMap<string, string>,
  manifest: ReadonlyArray<Decl>,
  markers: ReadonlyArray<Marker>,
  request: AckRequest
): { readonly ported: Map<string, string>; readonly acked: Array<string>; readonly errors: Array<string> } => {
  const entries = new Map(manifest.map((d) => [declId(d.path, d.name), d]))
  const marked = new Set(markers.map((m) => declId(m.path, m.name)).filter((id) => entries.has(id)))
  const next = new Map(ported)
  const acked: Array<string> = []
  const errors: Array<string> = []
  const ack = (id: string) => {
    next.set(id, entries.get(id)!.hash)
    acked.push(id)
  }
  for (const id of request.ids) {
    if (!entries.has(id)) errors.push(`'${id}' is not in porting/manifest.tsv`)
    else if (!marked.has(id)) errors.push(`'${id}' has no marker under crates/: add the marker first`)
    else ack(id)
  }
  for (const prefix of request.prefixes) {
    const matching = manifest.filter((d) => d.path.startsWith(prefix) && marked.has(declId(d.path, d.name)))
    if (matching.length === 0) errors.push(`no marked declaration under '${prefix}'`)
    matching.forEach((d) => ack(declId(d.path, d.name)))
  }
  if (request.prune) {
    for (const id of [...next.keys()]) if (!marked.has(id)) next.delete(id)
  }
  return { ported: next, acked, errors }
}

/** Parses `ack` positional arguments: pairs of `<path> <name>`, or single `<path>:<line> <name>` marker forms. */
export const parseAckArguments = (args: ReadonlyArray<string>): Array<string> => {
  if (args.length % 2 !== 0) {
    throw new PortError({ reason: "ack takes pairs of `<path> <name>`" })
  }
  const ids: Array<string> = []
  for (let i = 0; i < args.length; i += 2) {
    ids.push(declId(args[i]!.replace(/\.go:\d+$/, ".go"), args[i + 1]!))
  }
  return ids
}

// --- status -----------------------------------------------------------------------------------------------------

export interface Progress {
  code: number
  codeDone: number
  codeLines: number
  codeLinesDone: number
  test: number
  testDone: number
  testLines: number
  testLinesDone: number
}

const emptyProgress = (): Progress => ({
  code: 0,
  codeDone: 0,
  codeLines: 0,
  codeLinesDone: 0,
  test: 0,
  testDone: 0,
  testLines: 0,
  testLinesDone: 0
})

export const progressBy = (
  manifest: ReadonlyArray<Decl>,
  done: ReadonlySet<string>,
  keyOf: (d: Decl) => string
): Map<string, Progress> => {
  const rows = new Map<string, Progress>()
  for (const d of manifest) {
    const key = keyOf(d)
    let p = rows.get(key)
    if (p === undefined) rows.set(key, p = emptyProgress())
    const isDone = done.has(declId(d.path, d.name))
    if (isTestKind(d.kind)) {
      p.test++
      p.testLines += d.lines
      if (isDone) (p.testDone++, p.testLinesDone += d.lines)
    } else {
      p.code++
      p.codeLines += d.lines
      if (isDone) (p.codeDone++, p.codeLinesDone += d.lines)
    }
  }
  return rows
}

export const totalProgress = (rows: Iterable<Progress>) => {
  const total = emptyProgress()
  for (const p of rows) {
    for (const key of Object.keys(total) as Array<keyof Progress>) total[key] += p[key]
  }
  return total
}

const percent = (a: number, b: number) => (b === 0 ? "   -" : `${Math.floor((100 * a) / b)}%`.padStart(4))

export const statusTable = (rows: ReadonlyMap<string, Progress>, label: string) => {
  const sorted = [...rows].sort(([a], [b]) => compare(a, b))
  const width = Math.max(label.length, 5, ...sorted.map(([key]) => key.length)) + 2
  const lines = [
    `${label.padEnd(width)}${"code decls".padStart(16)} ${"code lines".padStart(20)} ${"tests".padStart(16)}`
  ]
  for (const [key, p] of [...sorted, ["TOTAL", totalProgress(rows.values())] as const]) {
    lines.push(
      key.padEnd(width) +
        `${p.codeDone}/${p.code}`.padStart(11) + ` ${percent(p.codeDone, p.code)}` +
        `${p.codeLinesDone}/${p.codeLines}`.padStart(15) + ` ${percent(p.codeLinesDone, p.codeLines)}` +
        `${p.testDone}/${p.test}`.padStart(11) + ` ${percent(p.testDone, p.test)}`
    )
  }
  return lines.join("\n")
}

export const statusMarkdown = (rows: ReadonlyMap<string, Progress>) => {
  const sorted = [...rows].sort(([a], [b]) => compare(a, b))
  const total = totalProgress(rows.values())
  const cell = (a: number, b: number) => `${a}/${b} (${percent(a, b).trim()})`
  return [
    "### Rust port progress",
    "",
    "| package | crate | code decls | code lines | tests |",
    "| --- | --- | ---: | ---: | ---: |",
    ...[...sorted, ["**TOTAL**", total] as const].map(([key, p]) =>
      `| ${key} | ${key.startsWith("**") ? "" : `\`${crateFor(key)}\``} | ${cell(p.codeDone, p.code)} | ${
        cell(p.codeLinesDone, p.codeLines)
      } | ${cell(p.testDone, p.test)} |`
    ),
    ""
  ].join("\n")
}

// --- imports (deps, ready) --------------------------------------------------------------------------------------

/** Packages of `module` (relative paths) imported by a Go source file. */
export const parseImports = (source: string, module: string): Array<string> => {
  const block = /^import\s*\(([\s\S]*?)^\)/m.exec(source)
  const text = block !== null ? block[1]! : source
  const found: Array<string> = []
  for (const m of text.matchAll(/^\s*(?:import\s+)?(?:[\w.]+\s+)?"([^"]+)"/gm)) {
    if (m[1]!.startsWith(module + "/")) found.push(m[1]!.slice(module.length + 1))
  }
  return found
}

export const parseModulePath = (goMod: string) => {
  const m = /^module\s+(\S+)/m.exec(goMod)
  if (m === null) throw new PortError({ reason: "go.mod has no module line" })
  return m[1]!
}

// --- diff -------------------------------------------------------------------------------------------------------

export interface DiffEntry {
  readonly decl: Decl
  /** For a moved declaration, its old path. */
  readonly from?: string
  /** The id the port status applies to (the old id for moves and removals). */
  readonly statusId: string
}

export const diffManifests = (oldDecls: ReadonlyArray<Decl>, newDecls: ReadonlyArray<Decl>) => {
  const old = new Map(oldDecls.map((d) => [declId(d.path, d.name), d]))
  const neu = new Map(newDecls.map((d) => [declId(d.path, d.name), d]))
  const removed = new Set([...old.keys()].filter((id) => !neu.has(id)))
  const added = new Set([...neu.keys()].filter((id) => !old.has(id)))
  // A declaration whose name and text are unchanged but whose file moved needs only its marker updated.
  const byText = new Map<string, string>()
  for (const id of removed) byText.set(`${old.get(id)!.name} ${old.get(id)!.hash}`, id)
  const moved: Array<DiffEntry> = []
  for (const id of [...added].sort(compare)) {
    const d = neu.get(id)!
    const key = `${d.name} ${d.hash}`
    const source = byText.get(key)
    if (source !== undefined) {
      byText.delete(key)
      removed.delete(source)
      added.delete(id)
      moved.push({ decl: d, from: old.get(source)!.path, statusId: source })
    }
  }
  const sortedIds = (ids: Iterable<string>) => [...ids].sort(compare)
  return {
    removed: sortedIds(removed).map((id) => ({ decl: old.get(id)!, statusId: id })),
    moved,
    changed: sortedIds(neu.keys())
      .filter((id) => old.has(id) && old.get(id)!.hash !== neu.get(id)!.hash)
      .map((id) => ({ decl: neu.get(id)!, statusId: id })),
    added: sortedIds(added).map((id) => ({ decl: neu.get(id)!, statusId: id }))
  }
}

// ---------------------------------------------------------------------------------------------------------------
// Repository access
// ---------------------------------------------------------------------------------------------------------------

const portingFile = (path: Path.Path, root: string, name: string) => path.join(root, "porting", name)

const readOptional = Effect.fnUntraced(function*(file: string) {
  const fs = yield* FileSystem.FileSystem
  return (yield* fs.exists(file)) ? yield* fs.readFileString(file) : ""
})

const readScope = Effect.fnUntraced(function*(root: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const text = yield* fs.readFileString(portingFile(path, root, "scope.toml"))
  return yield* Effect.try({
    try: () => parseScope(text),
    catch: (error) => error instanceof PortError ? error : new PortError({ reason: String(error) })
  })
})

const skipDirectory = (name: string) =>
  name.startsWith(".") || name.startsWith("_") || name === "testdata" || name === "node_modules" ||
  name === "crates" || name === "target"

/** Every Go package directory under `tree` (relative, "/"-separated), pruning excluded `/...` subtrees. */
const goPackages = Effect.fnUntraced(function*(tree: string, scope: Scope) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const found: Array<string> = []
  const walk = (rel: string): Effect.Effect<void, unknown> =>
    Effect.gen(function*() {
      if (rel !== "." && scope.exclude.some((e) => e.endsWith("/...") && matchesEntry(rel, e))) return
      const directory = rel === "." ? tree : path.join(tree, rel)
      const names = (yield* fs.readDirectory(directory)).sort(compare)
      let hasGo = false
      for (const name of names) {
        const info = yield* fs.stat(path.join(directory, name))
        if (info.type === "Directory") {
          if (!skipDirectory(name)) yield* walk(rel === "." ? name : `${rel}/${name}`)
        } else if (name.endsWith(".go")) {
          hasGo = true
        }
      }
      if (hasGo) found.push(rel)
    })
  yield* walk(".")
  return found.sort(compare)
})

const listDeclarations = Effect.fnUntraced(function*(repositoryRoot: string, tree: string, scope: Scope) {
  const path = yield* Path.Path
  const packages = (yield* goPackages(tree, scope)).filter((pkg) => classifyPackage(scope, pkg) === "included")
  const output = yield* runCommandString(
    "go",
    path.join(repositoryRoot, "_tools", "port_manifest"),
    ["run", ".", "-root", path.resolve(tree), ...packages],
    { GOWORK: "off", CGO_ENABLED: "0" }
  )
  const raw = output.split("\n").filter((line) => line.trim() !== "").map((line) => JSON.parse(line) as RawDecl)
  return buildManifest(raw, scope)
})

const readManifest = Effect.fnUntraced(function*(root: string) {
  const path = yield* Path.Path
  return parseManifest(yield* readOptional(portingFile(path, root, "manifest.tsv")))
})

const readSkipped = Effect.fnUntraced(function*(root: string) {
  const path = yield* Path.Path
  return parseSkipped(yield* readOptional(portingFile(path, root, "skipped.tsv")))
})

const readPorted = Effect.fnUntraced(function*(root: string) {
  const path = yield* Path.Path
  return parsePorted(yield* readOptional(portingFile(path, root, "ported.tsv")))
})

/** Every .rs file under crates/ (relative, "/"-separated). */
const rustFiles = Effect.fnUntraced(function*(root: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const found: Array<string> = []
  const walk = (rel: string): Effect.Effect<void, unknown> =>
    Effect.gen(function*() {
      const directory = path.join(root, rel)
      for (const name of (yield* fs.readDirectory(directory)).sort(compare)) {
        const info = yield* fs.stat(path.join(directory, name))
        if (info.type === "Directory") {
          if (name !== "target" && !name.startsWith(".")) yield* walk(`${rel}/${name}`)
        } else if (name.endsWith(".rs")) {
          found.push(`${rel}/${name}`)
        }
      }
    })
  if (yield* fs.exists(path.join(root, "crates"))) yield* walk("crates")
  return found
})

const readMarkers = Effect.fnUntraced(function*(root: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const markers: Array<Marker> = []
  for (const file of yield* rustFiles(root)) {
    markers.push(...parseMarkers(yield* fs.readFileString(path.join(root, file)), file))
  }
  return markers
})

const doneIds = Effect.fnUntraced(function*(root: string) {
  const markers = yield* readMarkers(root)
  const skipped = yield* readSkipped(root)
  return new Set([...markers.map((m) => declId(m.path, m.name)), ...skipped.keys()])
})

// ---------------------------------------------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------------------------------------------

export const portManifest = Effect.fnUntraced(function*(root: string, write: boolean) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const text = manifestText(yield* listDeclarations(root, root, yield* readScope(root)))
  if (write) {
    yield* fs.writeFileString(portingFile(path, root, "manifest.tsv"), text)
  } else {
    yield* Console.log(text.trimEnd())
  }
})

export const portStatus = Effect.fnUntraced(function*(root: string, prefix: string, files: boolean) {
  const manifest = (yield* readManifest(root)).filter((d) => d.path.startsWith(prefix))
  const rows = progressBy(manifest, yield* doneIds(root), (d) => (files ? d.path : packageOf(d.path)))
  yield* Console.log(statusTable(rows, files ? "file" : "package"))
})

export const portTodo = Effect.fnUntraced(function*(root: string, prefix: string) {
  const done = yield* doneIds(root)
  for (const d of yield* readManifest(root)) {
    if (d.path.startsWith(prefix) && !done.has(declId(d.path, d.name))) {
      yield* Console.log(`${markerText(d)}\t${d.kind}\t${d.lines} lines`)
    }
  }
})

export const portCheck = Effect.fnUntraced(function*(root: string, summary?: string) {
  const path = yield* Path.Path
  const scope = yield* readScope(root)
  const errors: Array<string> = []
  for (const pkg of yield* goPackages(root, scope)) {
    if (classifyPackage(scope, pkg) === "unclassified") {
      errors.push(`porting/scope.toml: Go package '${pkg}' is neither included nor excluded`)
    }
  }
  const manifestFile = portingFile(path, root, "manifest.tsv")
  const fresh = manifestText(yield* listDeclarations(root, root, scope))
  if ((yield* readOptional(manifestFile)) !== fresh) {
    errors.push("porting/manifest.tsv is stale: run `repoctl tsrs port-manifest --write`")
  }
  const manifest = parseManifest(fresh)
  const markers = yield* readMarkers(root)
  const skipped = yield* readSkipped(root)
  errors.push(...checkPort({
    manifest,
    markers,
    skipped,
    ported: yield* readPorted(root),
    requireNew: scope.requireNew
  }))
  if (summary !== undefined) {
    const done = new Set([...markers.map((m) => declId(m.path, m.name)), ...skipped.keys()])
    const markdown = statusMarkdown(progressBy(manifest, done, (d) => packageOf(d.path)))
    yield* Effect.tryPromise(() => appendFile(summary, markdown))
  }
  for (const error of errors) yield* Console.error(error)
  if (errors.length > 0) {
    return yield* new PortError({ reason: `port check failed with ${errors.length} error(s)` })
  }
  yield* Console.log(`Port bookkeeping OK (${manifest.length} declarations, ${markers.length} markers)`)
})

export const portFixLines = Effect.fnUntraced(function*(root: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const manifest = yield* readManifest(root)
  for (const file of yield* rustFiles(root)) {
    const absolute = path.join(root, file)
    const text = yield* fs.readFileString(absolute)
    const fixed = fixMarkerLines(text, manifest)
    if (fixed !== text) {
      yield* fs.writeFileString(absolute, fixed)
      yield* Console.log(file)
    }
  }
})

export const portAck = Effect.fnUntraced(function*(
  root: string,
  args: ReadonlyArray<string>,
  options: { readonly prefixes: ReadonlyArray<string>; readonly all: boolean; readonly prune: boolean }
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const ids = yield* Effect.try({
    try: () => parseAckArguments(args),
    catch: (error) => error instanceof PortError ? error : new PortError({ reason: String(error) })
  })
  const prefixes = options.all ? [""] : options.prefixes
  if (ids.length === 0 && prefixes.length === 0 && !options.prune) {
    return yield* new PortError({
      reason: "nothing to ack: pass `<path> <name>` pairs, --prefix <path prefix>, --all or --prune"
    })
  }
  const result = ackPorted(yield* readPorted(root), yield* readManifest(root), yield* readMarkers(root), {
    ids,
    prefixes,
    prune: options.prune
  })
  if (result.errors.length > 0) {
    for (const error of result.errors) yield* Console.error(error)
    return yield* new PortError({ reason: "ack failed; porting/ported.tsv was not changed" })
  }
  yield* fs.writeFileString(portingFile(path, root, "ported.tsv"), portedText(result.ported))
  yield* Console.log(`Acked ${result.acked.length} declaration(s)`)
})

export const portDiff = Effect.fnUntraced(function*(root: string, oldRef: string, newRef: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const scope = yield* readScope(root)
  const manifestAt = (ref: string, tmp: string, name: string) =>
    Effect.gen(function*() {
      const tree = path.join(tmp, name)
      const archive = path.join(tmp, `${name}.tar`)
      yield* fs.makeDirectory(tree, { recursive: true })
      yield* runCommand("git", root, ["archive", "--format=tar", "-o", archive, ref])
      yield* runCommand("tar", tmp, ["-xf", archive, "-C", tree])
      return yield* listDeclarations(root, tree, scope)
    })
  const { oldDecls, newDecls } = yield* Effect.scoped(Effect.gen(function*() {
    const tmp = yield* fs.makeTempDirectoryScoped({ prefix: "repoctl-port-diff-" })
    return { oldDecls: yield* manifestAt(oldRef, tmp, "old"), newDecls: yield* manifestAt(newRef, tmp, "new") }
  }))
  const markers = new Set((yield* readMarkers(root)).map((m) => declId(m.path, m.name)))
  const skipped = yield* readSkipped(root)
  const state = (id: string) => (markers.has(id) ? "ported" : skipped.has(id) ? "skipped" : "unported")
  const diff = diffManifests(oldDecls, newDecls)
  yield* Console.log(`# ${oldRef}..${newRef}; Go diff of a file: git diff ${oldRef} ${newRef} -- <file>`)
  yield* Console.log("# columns: location, name, kind, size, port status (at the old path for moves and removals)")
  const sections: ReadonlyArray<readonly [string, ReadonlyArray<DiffEntry>]> = [
    ["removed (delete the Rust port)", diff.removed],
    ["moved (update the marker path)", diff.moved],
    ["changed (re-port, then `repoctl tsrs port-ack`)", diff.changed],
    ["added (port)", diff.added]
  ]
  for (const [title, entries] of sections) {
    yield* Console.log(`## ${title}: ${entries.length}`)
    for (const { decl: d, from, statusId } of entries) {
      yield* Console.log(
        `${d.path}:${d.line}\t${d.name}\t${d.kind}\t${d.lines} lines\t${state(statusId)}${
          from === undefined ? "" : `\tfrom ${from}`
        }`
      )
    }
  }
})

export const portCrate = Effect.fnUntraced(function*(packages: ReadonlyArray<string>) {
  for (const pkg of packages) yield* Console.log(`${pkg}\t${crateFor(pkg)}`)
})

const packageImports = Effect.fnUntraced(function*(root: string, pkg: string, known: ReadonlySet<string>) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const module = parseModulePath(yield* fs.readFileString(path.join(root, "go.mod")))
  const directory = path.join(root, pkg)
  const found = new Set<string>()
  for (const name of yield* fs.readDirectory(directory)) {
    if (!name.endsWith(".go") || name.endsWith("_test.go")) continue
    for (const dep of parseImports(yield* fs.readFileString(path.join(directory, name)), module)) {
      if (known.has(dep) && dep !== pkg) found.add(dep)
    }
  }
  return [...found].sort(compare)
})

const packageProgress = Effect.fnUntraced(function*(root: string) {
  return progressBy(yield* readManifest(root), yield* doneIds(root), (d) => packageOf(d.path))
})

export const portDeps = Effect.fnUntraced(function*(root: string, pkg: string) {
  const progress = yield* packageProgress(root)
  for (const dep of yield* packageImports(root, pkg, new Set(progress.keys()))) {
    const p = progress.get(dep)!
    yield* Console.log(
      `${dep}\t${crateFor(dep)}\t${p.codeDone === p.code ? "done" : "NOT DONE"}\t${p.codeDone}/${p.code} decls`
    )
  }
})

export const portReady = Effect.fnUntraced(function*(root: string) {
  const progress = yield* packageProgress(root)
  const known = new Set(progress.keys())
  const rows: Array<readonly [string, Progress]> = []
  for (const [pkg, p] of progress) {
    if (p.codeDone === p.code && p.testDone === p.test) continue
    const deps = yield* packageImports(root, pkg, known)
    if (deps.every((dep) => progress.get(dep)!.codeDone === progress.get(dep)!.code)) rows.push([pkg, p])
  }
  yield* Console.log("# package\tcrate\tcode todo\ttests todo  (in-scope imports all ported; largest first)")
  rows.sort(([, a], [, b]) => (b.codeLines - b.codeLinesDone) - (a.codeLines - a.codeLinesDone))
  for (const [pkg, p] of rows) {
    yield* Console.log(
      `${pkg}\t${crateFor(pkg)}\t${p.code - p.codeDone} decls, ${p.codeLines - p.codeLinesDone} lines\t${
        p.test - p.testDone
      }`
    )
  }
})
