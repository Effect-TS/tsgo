import assert from "node:assert/strict"
import test from "node:test"
import { invalidateVendorHash, updateFlakeInputs, updateVendorHash } from "../src/flake.ts"

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
