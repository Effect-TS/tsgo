import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

const root = fileURLToPath(new URL("./", import.meta.url))
const compiler = resolve(process.argv[2] ?? join(root, "../../tsgo"))
const directory = mkdtempSync(join(root, ".verify-"))
const schemaPath = join(directory, "_schema.json")
const configPath = join(directory, "tsconfig.json")
const config = JSON.parse(readFileSync(join(root, "tsconfig.json"), "utf8"))
const originalSchema = readFileSync(join(root, "_schema.json"), "utf8")

const run = (command: string, args: Array<string>) => {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8", timeout: 30_000 })
  if (result.error) throw result.error
  assert.equal(result.signal, null, result.stderr)
  return { status: result.status, output: result.stdout + result.stderr }
}

const configure = (enabled: boolean) => {
  writeFileSync(configPath, JSON.stringify({
    ...config,
    compilerOptions: {
      ...config.compilerOptions,
      incremental: true,
      tsBuildInfoFile: join(directory, "check.tsbuildinfo"),
      plugins: [{
        name: "@effect/language-service",
        diagnostics: false,
        experimentalLiteralTemplateStrings: enabled
      }]
    },
    include: [],
    files: ["query.ts"]
  }))
}

let previousSource: string | undefined
const check = (body: string) => {
  const source = [
    "import * as Effect from 'effect/Effect'",
    "import * as SqlClient from 'effect/sql/SqlClient'",
    "import schema from './_schema.json' with { type: 'json' }",
    "import { tagged } from '../tagged.ts'",
    "class Database extends SqlClient.Service<Database>()('verify/Database', { schema }) {}",
    "export const program = Effect.gen(function*() {",
    "  const sql = tagged(yield* Database)",
    body,
    "})"
  ].join("\n")
  if (source !== previousSource) {
    writeFileSync(join(directory, "query.ts"), source)
    previousSource = source
  }
  return run(compiler, ["-p", configPath, "--pretty", "false"])
}
const accepts = (body: string) => {
  const result = check(body)
  assert.equal(result.status, 0, result.output)
}
const rejects = (body: string, message: RegExp) => {
  const result = check(body)
  assert.equal(result.status, 2, result.output)
  assert.match(result.output, message)
  assert.match(result.output, /query\.ts\(\d+,\d+\)/)
}

try {
  configure(true)
  writeFileSync(schemaPath, originalSchema)
  const valid = [
    "const minimumAge = 18",
    "const take = 2",
    "const rows = yield* sql`SELECT id AS userId, name, email FROM users WHERE age >= ${minimumAge} ORDER BY name LIMIT ${take}`",
    "rows[0].userId.toFixed()",
    "rows[0].name.toUpperCase()",
    "rows[0].email?.toLowerCase()"
  ].join("\n")
  accepts(valid)
  const joined = [
    "const rows = yield* sql`SELECT u.id, p.bio FROM users u LEFT JOIN profiles p ON u.id = p.user_id`",
    "rows[0].id.toFixed()",
    "rows[0].bio?.toUpperCase()"
  ].join("\n")
  accepts(joined)
  rejects(joined + "; rows[0].bio.toUpperCase()", /possibly 'null'/)
  accepts(joined.replace("LEFT JOIN", "INNER JOIN") + "; rows[0].bio.toUpperCase()")
  accepts("yield* sql`SELECT u.id FROM users u LEFT JOIN profiles p ON u.id = p.user_id AND p.bio = ${'Analytical Engine'} WHERE u.age >= ${18} LIMIT ${2}`")
  rejects("yield* sql`SELECT u.id FROM users u LEFT JOIN profiles p ON u.id = p.user_id AND p.bio = ${null}`", /null.*string/)
  rejects("yield* sql`SELECT id FROM users u LEFT JOIN profiles p ON u.id = p.user_id`", /Ambiguous column id/)
  accepts("const rows = yield* sql`SELECT id FROM users`; rows[0].id.toFixed()")
  accepts("const rows = yield* sql`SELECT\\u0020name FROM users`; rows[0].name.toUpperCase()")
  accepts("yield* sql`SELECT id FROM users WHERE email = ${null}`")
  accepts("const value: string = String.raw`unrelated ${1}`")
  accepts("const raw = yield* SqlClient.SqlClient; yield* raw`SELECT id FROM users WHERE id = ${1}`")
  rejects("yield* sql`SELECT missing FROM users`", /Unknown column missing/)
  rejects("yield* sql`SELECT id FROM missing`", /Unknown table missing/)
  rejects("yield* sql`SELECT id FROM users WHERE age >= ${'eighteen'}`", /string.*number/)
  rejects("yield* sql`SELECT id FROM users WHERE id = ${null}`", /null.*number/)
  rejects("yield* sql`SELECT ${'name'} FROM users`", /Unknown column/)
  rejects("yield* sql`SELECT id FROM users WHERE id = :id`", /Use interpolations/)
  rejects("yield* sql`SELECT id FROM users WHERE id = :p0 AND age >= ${18}`", /Use interpolations/)
  rejects("const rows = yield* sql`SELECT email FROM users`; rows[0].email.toLowerCase()", /possibly 'null'/)
  rejects("const rows = yield* sql`SELECT id FROM users`; rows[0].name", /Property 'name' does not exist/)
  rejects("const rows = yield* sql`SELECT id FROM users`; rows[0].id = 1", /read-only property/)

  // Keep the query and incremental state unchanged while modifying only its dependencies.
  accepts(valid)
  const changed = JSON.parse(originalSchema)
  changed.tables.users.name = { INTEGER: null }
  writeFileSync(schemaPath, JSON.stringify(changed))
  rejects(valid, /toUpperCase/)
  writeFileSync(schemaPath, originalSchema)
  accepts(valid)

  accepts(joined)
  const changedProfile = JSON.parse(originalSchema)
  changedProfile.tables.profiles.bio = { INTEGER: null }
  writeFileSync(schemaPath, JSON.stringify(changedProfile))
  rejects(joined, /toUpperCase/)
  writeFileSync(schemaPath, originalSchema)
  accepts(joined)
  configure(false)
  rejects(valid, /Expected 1 arguments|never/)
  configure(true)
  accepts(valid)

  writeFileSync(join(directory, "emit.ts"), [
    "declare function capture<const Parts extends readonly string[]>(strings: Parts): Parts",
    "export const text = capture`hello`[0]"
  ].join("\n"))
  for (const enabled of [true, false, true]) {
    const emitConfig = join(directory, "emit.tsconfig.json")
    writeFileSync(emitConfig, JSON.stringify({
      compilerOptions: {
        target: "ES2022",
        strict: true,
        types: [],
        incremental: true,
        declaration: true,
        emitDeclarationOnly: true,
        outDir: "emit",
        tsBuildInfoFile: "emit.tsbuildinfo",
        plugins: [{
          name: "@effect/language-service",
          diagnostics: false,
          experimentalLiteralTemplateStrings: enabled
        }]
      },
      files: ["emit.ts"]
    }))
    const emitted = run(compiler, ["-p", emitConfig, "--pretty", "false"])
    assert.equal(emitted.status, 0, emitted.output)
    assert.equal(
      readFileSync(join(directory, "emit/emit.d.ts"), "utf8").trim(),
      `export declare const text: ${enabled ? '"hello"' : "string"};`
    )
  }

  const demo = run(process.execPath, [join(root, "main.ts")])
  assert.equal(demo.status, 0, demo.output)
  assert.deepEqual(JSON.parse(demo.output), {
    sql: "SELECT u.id AS userId, u.name, u.email, p.bio AS profileBio FROM users AS u LEFT JOIN profiles AS p ON u.id = p.user_id WHERE u.age >= ? ORDER BY u.name LIMIT ?",
    parameters: [18, 2],
    cards: [
      { id: "1", name: "ADA", email: "(no email)", profile: "Analytical Engine" },
      { id: "2", name: "GRACE", email: "grace@example.com", profile: "(no profile)" }
    ],
    untrustedNameMatches: 0
  })
  console.log("Verified tagged-template inference, join nullability, bound parameters, schema and option invalidation, and SQLite execution.")
} finally {
  rmSync(directory, { recursive: true, force: true })
}
