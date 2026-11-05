import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import assert from "node:assert/strict"
import { existsSync, mkdirSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { parseByteSize, selectEvictions, trimDirectory } from "../src/goCache.ts"

const day = 24 * 60 * 60

// Writes `size` bytes at `path` (relative to root) with an mtime `age` days in the past.
const writeEntry = (root: string, path: string, size: number, age: number) => {
  const file = join(root, path)
  mkdirSync(join(file, ".."), { recursive: true })
  writeFileSync(file, Buffer.alloc(size))
  const time = Date.now() / 1000 - age * day
  utimesSync(file, time, time)
}

const trim = (root: string, maxBytes: bigint) =>
  Effect.runPromise(trimDirectory(root, maxBytes).pipe(Effect.provide(NodeServices.layer)))

const withCache = async (body: (root: string) => Promise<void>) => {
  const root = mkdtempSync(join(tmpdir(), "repoctl-go-cache-"))
  try {
    await body(root)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

test("keeps the most recently used files within the budget", () =>
  withCache(async (root) => {
    writeEntry(root, "README", 10, 30)
    writeEntry(root, "trim.txt", 10, 30)
    writeEntry(root, "aa/aa01-a", 100, 1)
    writeEntry(root, "bb/bb01-d", 200, 2)
    writeEntry(root, "cc/cc01-d", 300, 3)
    writeEntry(root, "dd/dd01-a", 50, 4)

    // 20 bytes of protected files + 100 + 200 fit in 330; the 300-byte entry does not, the 50-byte one still does
    const result = await trim(root, 370n)

    assert.deepEqual(result, { bytesBefore: 670n, bytesAfter: 370n, filesBefore: 6, filesRemoved: 1 })
    assert.ok(existsSync(join(root, "README")))
    assert.ok(existsSync(join(root, "trim.txt")))
    assert.ok(existsSync(join(root, "aa/aa01-a")))
    assert.ok(existsSync(join(root, "bb/bb01-d")))
    assert.ok(!existsSync(join(root, "cc/cc01-d")))
    assert.ok(existsSync(join(root, "dd/dd01-a")))
  }))

test("never removes README and trim.txt, even over budget", () =>
  withCache(async (root) => {
    writeEntry(root, "README", 10, 30)
    writeEntry(root, "trim.txt", 10, 30)
    writeEntry(root, "aa/aa01-a", 100, 0)

    const result = await trim(root, 5n)

    assert.deepEqual(result, { bytesBefore: 120n, bytesAfter: 20n, filesBefore: 3, filesRemoved: 1 })
    assert.ok(existsSync(join(root, "README")))
    assert.ok(existsSync(join(root, "trim.txt")))
    assert.ok(!existsSync(join(root, "aa/aa01-a")))
  }))

test("is a no-op under the budget", () =>
  withCache(async (root) => {
    writeEntry(root, "README", 10, 30)
    writeEntry(root, "aa/aa01-a", 100, 1)
    writeEntry(root, "bb/bb01-d", 200, 2)

    const result = await trim(root, 1000n)

    assert.deepEqual(result, { bytesBefore: 310n, bytesAfter: 310n, filesBefore: 3, filesRemoved: 0 })
    assert.ok(existsSync(join(root, "aa/aa01-a")))
    assert.ok(existsSync(join(root, "bb/bb01-d")))
  }))

test("evicts the oldest files first", () => {
  const files = [
    { path: "old", size: 10n, mtimeMs: 1 },
    { path: "new", size: 10n, mtimeMs: 3 },
    { path: "mid", size: 10n, mtimeMs: 2 }
  ]
  assert.deepEqual(selectEvictions(files, 20n).map((file) => file.path), ["old"])
  assert.deepEqual(selectEvictions(files, 0n).map((file) => file.path), ["new", "mid", "old"])
})

test("parses byte sizes", () => {
  assert.deepEqual(parseByteSize("2GB"), Option.some(2_000_000_000n))
  assert.deepEqual(parseByteSize("1 GiB"), Option.some(1_073_741_824n))
  assert.ok(Option.isNone(parseByteSize("lots")))
})
