import * as NodeServices from "@effect/platform-node/NodeServices"
import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import * as Effect from "effect/Effect"
import {
  cargoLockPathVersions,
  parseCargoPackage,
  parseCargoWorkspaceVersion,
  requireTsrsCheckout,
  runRustChecks,
  runRustLint,
  runRustTests,
  syncCargoLockVersions,
  tsrsCheckedOut
} from "../src/rust.ts"

const run = <A, E>(effect: Effect.Effect<A, E, NodeServices.NodeServices>) =>
  Effect.runPromise(effect.pipe(Effect.provide(NodeServices.layer)))

test("the tsrs cargo steps fail without a tsrs checkout, before running cargo", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-tsrs-"))
  try {
    assert.equal(await run(tsrsCheckedOut(repository)), false)
    for (const step of [requireTsrsCheckout, runRustChecks, runRustLint, runRustTests]) {
      await assert.rejects(
        run(step(repository)),
        /tsrs\/ is not checked out: run `pnpm exec repoctl submodules setup --component tsrs`/
      )
    }

    mkdirSync(join(repository, "tsrs"))
    writeFileSync(join(repository, "tsrs", "Cargo.toml"), "[workspace]\n")
    assert.equal(await run(tsrsCheckedOut(repository)), true)
    await run(requireTsrsCheckout(repository))
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})

test("reads a Cargo package's name and version, inheriting the workspace version", () => {
  const workspace = "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nedition = \"2024\"\nversion = \"7.1.0-beta.tsrs2\"\n"
  assert.equal(parseCargoWorkspaceVersion(workspace), "7.1.0-beta.tsrs2")
  assert.deepEqual(
    parseCargoPackage("[package]\nname = \"tsrs_ast\"\nversion.workspace = true\n\n[dependencies]\nversion = \"9\"\n", "1.0.0"),
    { name: "tsrs_ast", version: "1.0.0" }
  )
  assert.deepEqual(parseCargoPackage("[package]\nname = \"tsrs_vfs\"\nversion = \"7.1.0-beta.tsrs2\"\n", "1.0.0"), {
    name: "tsrs_vfs",
    version: "7.1.0-beta.tsrs2"
  })
  assert.equal(parseCargoPackage("[workspace]\nmembers = []\n", "1.0.0"), undefined)
})

test("sets the versions of path packages in Cargo.lock only", () => {
  const lock = [
    "version = 4",
    "",
    "[[package]]",
    "name = \"tsrs_ast\"",
    "version = \"0.25.0\"",
    "dependencies = [",
    " \"bitflags\",",
    "]",
    "",
    "[[package]]",
    "name = \"bitflags\"",
    "version = \"2.9.0\"",
    "source = \"registry+https://github.com/rust-lang/crates.io-index\"",
    "",
    "[[package]]",
    "name = \"ets_shim\"",
    "version = \"0.0.0\"",
    ""
  ].join("\n")
  const synced = syncCargoLockVersions(lock, new Map([["tsrs_ast", "7.1.0-beta.tsrs2"], ["bitflags", "3.0.0"]]))
  assert.equal(synced, lock.replace("version = \"0.25.0\"", "version = \"7.1.0-beta.tsrs2\""))
  assert.equal(syncCargoLockVersions(synced, new Map([["tsrs_ast", "7.1.0-beta.tsrs2"]])), synced)
})

test("reads the versions of the tsrs path packages of a Cargo.lock", () => {
  const lock = [
    "version = 4",
    "",
    "[[package]]",
    "name = \"tsrs_ast\"",
    "version = \"0.25.0\"",
    "",
    "[[package]]",
    "name = \"tsrs_lsp_types\"",
    "version = \"0.1.0\"",
    "source = \"registry+https://github.com/rust-lang/crates.io-index\"",
    "",
    "[[package]]",
    "name = \"ets_shim\"",
    "version = \"0.0.0\"",
    ""
  ].join("\n")
  assert.deepEqual(
    [...cargoLockPathVersions(lock, (name) => name.startsWith("tsrs"))],
    [["tsrs_ast", "0.25.0"]]
  )
  // A tsrs setup of another release and the restore of the committed versions round-trip.
  const other = syncCargoLockVersions(lock, new Map([["tsrs_ast", "7.1.0-beta.tsrs2"]]))
  assert.notEqual(other, lock)
  assert.equal(syncCargoLockVersions(other, cargoLockPathVersions(lock, (name) => name.startsWith("tsrs"))), lock)
})
