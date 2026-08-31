import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import { join } from "node:path"
import test from "node:test"
import { fileURLToPath } from "node:url"
import { invalidateVendorHash, updateFlakeInputs, updateVendorHash } from "../src/flake.ts"

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url))
const oldRevision = "0123456789abcdef0123456789abcdef01234567"
const newTsRevision = "2123456789abcdef0123456789abcdef01234567"

test("updates pinned flake inputs", () => {
  const flake = [
    `url = "github:microsoft/TypeScript/${oldRevision}";`
  ].join("\n")

  assert.equal(updateFlakeInputs(flake, newTsRevision), [
    `url = "github:microsoft/TypeScript/${newTsRevision}";`
  ].join("\n"))
})

test("keeps the pinned flake input synchronized with TypeScript next", () => {
  const upstream = JSON.parse(
    readFileSync(join(repositoryRoot, "_packages", "tsgo", "upstream.json"), "utf8")
  ) as {
    readonly tags: { readonly typescript: { readonly next: string } }
    readonly components: {
      readonly typescript: Readonly<Record<string, { readonly gitHead: string }>>
    }
  }
  const revision = upstream.components.typescript[upstream.tags.typescript.next]?.gitHead
  assert.ok(revision)

  const flake = readFileSync(join(repositoryRoot, "flake.nix"), "utf8")
  assert.equal(updateFlakeInputs(flake, revision), flake)
})

test("updates the vendor hash", () => {
  assert.equal(
    updateVendorHash('vendorHash = "sha256-old";', "sha256-new"),
    'vendorHash = "sha256-new";'
  )
  assert.equal(
    updateVendorHash("vendorHash = lib.fakeHash;", "sha256-new"),
    'vendorHash = "sha256-new";'
  )
})

test("invalidates the vendor hash", () => {
  assert.equal(
    invalidateVendorHash('vendorHash = "sha256-old";'),
    "vendorHash = lib.fakeHash;"
  )
})
