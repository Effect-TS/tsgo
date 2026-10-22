import assert from "node:assert/strict"
import { spawnSync } from "node:child_process"
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { arch, cpus, platform } from "node:os"
import { join, resolve } from "node:path"
import { performance } from "node:perf_hooks"
import { fileURLToPath } from "node:url"

const root = fileURLToPath(new URL("./", import.meta.url))
const compiler = resolve(process.argv[2] ?? join(root, "../../tsgo"))
const directory = mkdtempSync(join(root, ".verify-measure-"))
const config = JSON.parse(readFileSync(join(root, "tsconfig.json"), "utf8"))
const queryCount = 100
const samples = 8
const modes = ["parentheses", "tagged"]
const timings = { parentheses: [], tagged: [] }

const run = (args) => {
  const start = performance.now()
  const result = spawnSync(compiler, args, { cwd: root, encoding: "utf8", timeout: 60_000 })
  if (result.error) throw result.error
  assert.equal(result.status, 0, result.stdout + result.stderr)
  return { milliseconds: performance.now() - start, output: result.stdout.trim() }
}

try {
  copyFileSync(join(root, "_schema.json"), join(directory, "_schema.json"))
  for (const mode of modes) {
    const tagged = mode === "tagged"
    const source = [
      "import * as Effect from 'effect/Effect'",
      "import * as SqlClient from 'effect/sql/SqlClient'",
      "import schema from './_schema.json' with { type: 'json' }",
      ...(tagged ? ["import { tagged } from '../tagged.ts'"] : []),
      "class Database extends SqlClient.Service<Database>()('measure/Database', { schema }) {}",
      "export const program = Effect.gen(function*() {",
      tagged ? "const sql = tagged(yield* Database)" : "const sql = yield* Database",
      "const minimumAge = 18; const limit = 2"
    ]
    for (let i = 0; i < queryCount; i++) {
      const text = "SELECT id AS user" + i + ", name, email FROM users WHERE age >= "
      const call = tagged
        ? "sql`" + text + "${minimumAge} ORDER BY name LIMIT ${limit}`"
        : "sql(" + JSON.stringify(text + ":minimumAge ORDER BY name LIMIT :limit") + ", { minimumAge, limit })"
      source.push("const rows" + i + " = yield* " + call)
      source.push("rows" + i + "[0].user" + i + ".toFixed(); rows" + i + "[0].email?.toLowerCase()")
    }
    source.push("})")
    writeFileSync(join(directory, mode + ".ts"), source.join("\n"))
    writeFileSync(join(directory, mode + ".json"), JSON.stringify({
      ...config,
      compilerOptions: {
        ...config.compilerOptions,
        incremental: false,
        plugins: [{ name: "@effect/language-service", diagnostics: false, experimentalLiteralTemplateStrings: tagged }]
      },
      include: [],
      files: [mode + ".ts"]
    }))
  }
  for (let round = -2; round < samples; round++) {
    for (const mode of round % 2 === 0 ? modes : modes.toReversed()) {
      const result = run(["-p", join(directory, mode + ".json"), "--pretty", "false"])
      if (round >= 0) timings[mode].push(result.milliseconds)
    }
  }
  const summarize = (values) => {
    const sorted = values.toSorted((a, b) => a - b)
    return {
      medianMs: Math.round((sorted[samples / 2 - 1] + sorted[samples / 2]) / 2),
      minMs: Math.round(sorted[0]),
      maxMs: Math.round(sorted.at(-1)),
      samplesMs: values.map(Math.round)
    }
  }
  console.log(JSON.stringify({
    compiler: run(["--version"]).output,
    environment: { platform: platform(), arch: arch(), cpu: cpus()[0]?.model, node: process.version },
    workload: { queryCount, samples, warmups: 2, incremental: false, interleaved: true },
    parentheses: summarize(timings.parentheses),
    tagged: summarize(timings.tagged)
  }, null, 2))
} finally {
  rmSync(directory, { recursive: true, force: true })
}
