import { spawnSync } from "node:child_process"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
import { build } from "tsdown"
import { afterAll, beforeAll, describe, expect, it } from "vitest"

describe.skipIf(process.platform === "win32" || typeof process.execve !== "function")("proxy native stdio", () => {
  const payload = Buffer.alloc(2 * 1024 * 1024, "x")
  let directory: string
  let proxyModule: string
  let payloadFile: string

  beforeAll(async () => {
    directory = await mkdtemp(join(tmpdir(), "effect-tsgo-proxy-stdio-"))
    payloadFile = join(directory, "payload")
    await writeFile(payloadFile, payload)
    await build({
      config: false,
      entry: { proxy: fileURLToPath(new URL("../src/cli/proxy/exec.ts", import.meta.url)) },
      outDir: directory,
      format: "esm",
      platform: "node",
      dts: false,
      clean: false,
      logLevel: "silent",
      outExtensions: () => ({ js: ".mjs" })
    })
    proxyModule = pathToFileURL(join(directory, "proxy.mjs")).href
  }, 30_000)

  afterAll(async () => {
    if (directory !== undefined) await rm(directory, { recursive: true, force: true })
  })

  it.each(["stdout", "stderr", "stdin"] as const)("preserves large native %s pipes", (stream) => {
    const command = stream === "stderr"
      ? { file: "/bin/sh", args: ["-c", "exec /bin/cat \"$1\" >&2", "sh", payloadFile] }
      : { file: "/bin/cat", args: stream === "stdin" ? [] : [payloadFile] }
    const script = `
      import { execProxyCommand } from ${JSON.stringify(proxyModule)}
      // The CLI runtime initializes these streams before handing off to the native tool.
      void process.stdin
      void process.stdout
      void process.stderr
      execProxyCommand({ ...${JSON.stringify(command)}, env: process.env })
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
    expect(result.stdout).toEqual(stream === "stderr" ? Buffer.alloc(0) : payload)
    expect(result.stderr).toEqual(stream === "stderr" ? payload : Buffer.alloc(0))
  }, 15_000)
})
