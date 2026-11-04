import { spawnSync } from "node:child_process"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { build } from "tsdown"
import { afterAll, beforeAll, describe, expect, it } from "vitest"

// Node makes piped stdio non-blocking once process.stdin/stdout/stderr is first read, and execve hands that to the
// tool, which then loses output once the pipe fills. The proxy's services must not read them.
describe.skipIf(process.platform === "win32" || typeof process.execve !== "function")("proxy native stdio", () => {
  const payload = Buffer.alloc(2 * 1024 * 1024, "x")
  let directory: string
  let fixtureModule: string
  let payloadFile: string

  beforeAll(async () => {
    directory = await mkdtemp(join(tmpdir(), "effect-tsgo-proxy-stdio-"))
    payloadFile = join(directory, "payload")
    await writeFile(payloadFile, payload)
    await build({
      config: false,
      entry: { fixture: fileURLToPath(new URL("./fixtures/proxy-stdio.ts", import.meta.url)) },
      outDir: directory,
      format: "esm",
      platform: "node",
      noExternal: [/./],
      dts: false,
      clean: false,
      logLevel: "silent",
      outExtensions: () => ({ js: ".mjs" })
    })
    fixtureModule = pathToFileURL(join(directory, "fixture.mjs")).href
  }, 60_000)

  afterAll(async () => {
    if (directory !== undefined) await rm(directory, { recursive: true, force: true })
  })

  it.each(["stdout", "stderr", "stdin"] as const)("preserves large native %s pipes", (stream) => {
    const command = stream === "stderr"
      ? { file: "/bin/sh", args: ["-c", "exec /bin/cat \"$1\" >&2", "sh", payloadFile] }
      : { file: "/bin/cat", args: stream === "stdin" ? [] : [payloadFile] }
    const script = `
      import { runWithProxyServices } from ${JSON.stringify(fixtureModule)}
      runWithProxyServices({ ...${JSON.stringify(command)}, env: process.env })
    `
    const result = spawnSync(process.execPath, ["--input-type=module", "-e", script], {
      env: { ...process.env, NODE_OPTIONS: "" },
      input: stream === "stdin" ? payload : undefined,
      maxBuffer: 4 * 1024 * 1024,
      timeout: 10_000
    })

    expect(result.error).toBeUndefined()
    expect(result.signal).toBeNull()
    expect(result.status).toBe(0)
    // Buffer.equals: a deep toEqual on 2 MiB takes seconds.
    const expectedStdout = stream === "stderr" ? Buffer.alloc(0) : payload
    const expectedStderr = stream === "stderr" ? payload : Buffer.alloc(0)
    expect(result.stdout.length).toBe(expectedStdout.length)
    expect(result.stderr.length).toBe(expectedStderr.length)
    expect(result.stdout.equals(expectedStdout)).toBe(true)
    expect(result.stderr.equals(expectedStderr)).toBe(true)
  }, 15_000)
})
