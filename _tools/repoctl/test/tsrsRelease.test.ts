import * as NodeServices from "@effect/platform-node/NodeServices"
import assert from "node:assert/strict"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import * as Effect from "effect/Effect"
import {
  boltToolsUrl,
  checkTsrsReleaseParity,
  effectWorkloadScript,
  parseLlvmVersion,
  parseTsrsPnpmVersion,
  parseTsrsReleaseTargets,
  parseTsrsTypeScriptCommit,
  tsrsBuildTargets,
  tsrsCargoBuildArgs,
  tsrsCargoEnvironment,
  tsrsReleaseLayout,
  tsrsReleaseParityProblems,
  tsrsReleaseWorkflowSnippets
} from "../src/tsrsRelease.ts"

const run = <A, E>(effect: Effect.Effect<A, E, NodeServices.NodeServices>) =>
  Effect.runPromise(effect.pipe(Effect.provide(NodeServices.layer)))

const layout = tsrsReleaseLayout("/work")

test("lays the release build's state out under its work directory", () => {
  assert.deepEqual(layout, {
    work: "/work",
    instrumentedTarget: "/work/pgo/target",
    rawProfiles: "/work/pgo/raw",
    bench: "/work/pgo/bench",
    effectWorkload: "/work/pgo/effect",
    pnpm: "/work/pnpm",
    bolt: "/work/bolt",
    llvm: "/work/llvm",
    determinismTarget: "/work/determinism-target"
  })
})

test("reads tsrs's pinned pnpm version from its packageManager", () => {
  assert.equal(parseTsrsPnpmVersion(`{"packageManager":"pnpm@12.15.0"}`), "12.15.0")
  assert.equal(parseTsrsPnpmVersion(`{"packageManager":"pnpm@12.15.0+sha512.abc"}`), "12.15.0")
  assert.equal(parseTsrsPnpmVersion(`{"packageManager":"yarn@4.0.0"}`), undefined)
  assert.equal(parseTsrsPnpmVersion(`{}`), undefined)
})

test("gives both cargo builds tsrs's RUSTFLAGS, strip setting and CFLAGS per target", () => {
  const generate = { kind: "generate", rawProfiles: layout.rawProfiles, targetDirectory: layout.instrumentedTarget } as const
  assert.deepEqual(tsrsCargoEnvironment({ target: "linux-x64", optimization: "pgo-bolt", phase: generate }), {
    CARGO_PROFILE_RELEASE_STRIP: "none",
    CARGO_ENCODED_RUSTFLAGS: "-Cprofile-generate=/work/pgo/raw\x1f-Clto=thin",
    CARGO_TARGET_DIR: "/work/pgo/target"
  })
  assert.deepEqual(
    tsrsCargoEnvironment({ target: "linux-arm64", optimization: "pgo-bolt", phase: { kind: "use", profile: "/p.profdata" } }),
    {
      CARGO_PROFILE_RELEASE_STRIP: "none",
      CARGO_ENCODED_RUSTFLAGS: "-Cprofile-use=/p.profdata\x1f-Clink-arg=-Wl,--emit-relocs"
    }
  )
  assert.deepEqual(tsrsCargoEnvironment({ target: "darwin-arm64", optimization: "pgo", phase: generate }), {
    CARGO_PROFILE_RELEASE_STRIP: "symbols",
    CARGO_ENCODED_RUSTFLAGS: "-Cprofile-generate=/work/pgo/raw\x1f-Clto=thin",
    CARGO_TARGET_DIR: "/work/pgo/target",
    CFLAGS: "-fno-profile-generate"
  })
  assert.deepEqual(
    tsrsCargoEnvironment({ target: "darwin-x64", optimization: "pgo", phase: { kind: "use", profile: "/p.profdata" } }),
    { CARGO_PROFILE_RELEASE_STRIP: "symbols", CARGO_ENCODED_RUSTFLAGS: "-Cprofile-use=/p.profdata", CFLAGS: "-fno-profile-use" }
  )
  // No C compiler and no CFLAGS on Windows (MSVC) or Linux (gcc): the runner's default compiler, as tsrs's jobs.
  assert.deepEqual(
    tsrsCargoEnvironment({ target: "win32-x64", optimization: "pgo", phase: { kind: "use", profile: "C:\\p.profdata" } }),
    { CARGO_PROFILE_RELEASE_STRIP: "symbols", CARGO_ENCODED_RUSTFLAGS: "-Cprofile-use=C:\\p.profdata" }
  )
})

test("builds the training binaries in the tsrs binary's cargo invocation", () => {
  assert.deepEqual(tsrsCargoBuildArgs("linux-x64", true), [
    "build",
    "--release",
    "--locked",
    "-p",
    "ets_tsrs",
    "-p",
    "ets_pgotrain",
    "-p",
    "tsrs_testrunner",
    "-p",
    "tsrs_fourslash_server",
    "--bins",
    "--features",
    "ets_tsrs/cli,tsrs_cli/noembed,ets_pgotrain/train",
    "--target",
    "x86_64-unknown-linux-gnu"
  ])
  assert.ok(!tsrsCargoBuildArgs("win32-x64", true).includes("tsrs_fourslash_server"))
  // The plain release build (rust.ts buildRustBinary) has the same packages and features.
  assert.deepEqual(tsrsCargoBuildArgs("darwin-arm64", false), [
    "build",
    "--release",
    "--locked",
    "-p",
    "ets_tsrs",
    "--features",
    "cli,tsrs_cli/noembed",
    "--target",
    "aarch64-apple-darwin"
  ])
})

test("fetches BOLT from the LLVM release of rustc's LLVM", () => {
  assert.equal(parseLlvmVersion("rustc 1.99.1\nhost: x86_64-unknown-linux-gnu\nLLVM version: 23.1.1\n"), "23.1.1")
  assert.equal(parseLlvmVersion("rustc 1.99.1\n"), undefined)
  assert.deepEqual(boltToolsUrl("23.1.1", "arm64"), {
    name: "LLVM-23.1.1-Linux-ARM64",
    url: "https://github.com/llvm/llvm-project/releases/download/llvmorg-23.1.1/LLVM-23.1.1-Linux-ARM64.tar.zst",
    files: [
      "LLVM-23.1.1-Linux-ARM64/bin/llvm-bolt",
      "LLVM-23.1.1-Linux-ARM64/bin/merge-fdata",
      "LLVM-23.1.1-Linux-ARM64/lib/libbolt_rt_instr.a"
    ]
  })
  assert.equal(boltToolsUrl("23.1.1", "x64").name, "LLVM-23.1.1-Linux-X64")
})

test("reads the TypeScript commit tsrs ports from its Cargo.toml", () => {
  const commit = "3e9236361fa97a9e6c685f4833ac457ae2d8c50a"
  assert.equal(
    parseTsrsTypeScriptCommit(
      `[workspace]\nmembers = []\n\n[workspace.metadata.typescript]\nversion = "7.1.0-beta"\ncommit = "${commit}"\n`
    ),
    commit
  )
  assert.equal(
    parseTsrsTypeScriptCommit(`[workspace.metadata.typescript]\r\nversion = "7.1.0-beta"\r\ncommit = "${commit}"\r\n`),
    commit
  )
  assert.equal(parseTsrsTypeScriptCommit(`[package]\ncommit = "${commit}"\n`), undefined)
})

test("builds the Effect workload from clean for bolt.sh, with tsrs's exit status", () => {
  assert.equal(
    effectWorkloadScript("/work/it's/effect"),
    [
      "#!/usr/bin/env bash",
      "# Generated by repoctl (tsrsRelease.ts): BOLT_EXTRA_WORKLOAD for tsrs/.github/scripts/bolt.sh.",
      "set -uo pipefail",
      "exe=$1 out=$2",
      "shift 2",
      "cd '/work/it'\\''s/effect' || exit 3",
      "pnpm clean > /dev/null || exit 3",
      `"$exe" -b tsconfig.json --pretty false "$@" > "$out"`,
      ""
    ].join("\n")
  )
})

// A release workflow with every mirrored setting and tsrs's targets.
const workflow = (rows: ReadonlyArray<string>, windowsRunner = "windows-latest") =>
  [
    "jobs:",
    "  build:",
    "    strategy:",
    "      matrix:",
    "        include:",
    ...rows.map((row) => `          - ${row}`),
    "    steps:",
    ...tsrsReleaseWorkflowSnippets.filter((snippet) => !snippet.startsWith("runs-on: ")).map((snippet) =>
      `      ${snippet}`),
    "",
    "  build-windows:",
    "    name: build x86_64-pc-windows-msvc",
    `    runs-on: ${windowsRunner}`,
    "",
    "  build-musl:",
    "    strategy:",
    "      matrix:",
    "        include:",
    "          - { target: x86_64-unknown-linux-musl, os: ubuntu-latest }",
    ""
  ].join("\n")

const upstreamRows = [
  "{ target: aarch64-apple-darwin, os: macos-latest, experimental: false, run: true }",
  "{ target: x86_64-apple-darwin, os: macos-26-intel, experimental: false, run: true }",
  "{ target: x86_64-unknown-linux-gnu, os: ubuntu-22.04, experimental: false, run: true }",
  "{ target: aarch64-unknown-linux-gnu, os: ubuntu-22.04-arm, experimental: false, run: true }"
]

test("matches tsrs's release targets, runners and build settings", () => {
  assert.deepEqual([...parseTsrsReleaseTargets(workflow(upstreamRows))], [
    ["aarch64-apple-darwin", "macos-latest"],
    ["x86_64-apple-darwin", "macos-26-intel"],
    ["x86_64-unknown-linux-gnu", "ubuntu-22.04"],
    ["aarch64-unknown-linux-gnu", "ubuntu-22.04-arm"],
    ["x86_64-unknown-linux-musl", "ubuntu-latest"],
    ["x86_64-pc-windows-msvc", "windows-latest"]
  ])
  assert.deepEqual(tsrsReleaseParityProblems(workflow(upstreamRows)), [])
  assert.equal(Object.keys(tsrsBuildTargets).length, 5)
})

test("reports how tsrs's release workflow moved away from the mirrored build", () => {
  const changed = workflow([
    "{ target: aarch64-apple-darwin, os: macos-27, experimental: false, run: true }",
    "{ target: x86_64-unknown-linux-gnu, os: ubuntu-22.04, experimental: false, run: true }",
    "{ target: aarch64-unknown-linux-gnu, os: ubuntu-22.04-arm, experimental: false, run: true }",
    "{ target: riscv64gc-unknown-linux-gnu, os: ubuntu-24.04-riscv, experimental: false, run: true }"
  ], "windows-2025")
    .replace("-Clto=thin\n", "\n")
    .replace("    steps:\n", "    env:\n      TARGET_CC: clang\n    steps:\n")
  assert.deepEqual(tsrsReleaseParityProblems(changed), [
    "no longer contains: RUSTFLAGS: -Cprofile-generate=${{ runner.temp }}/pgo/raw -Clto=thin",
    "no longer contains: runs-on: windows-latest",
    "sets a C compiler (CC or TARGET_CC)",
    "builds aarch64-apple-darwin on macos-27, tsrsBuildTargets on macos-latest",
    "builds riscv64gc-unknown-linux-gnu (on ubuntu-24.04-riscv), which tsrsBuildTargets lacks",
    "builds x86_64-pc-windows-msvc on windows-2025, tsrsBuildTargets on windows-latest",
    "no longer builds x86_64-apple-darwin"
  ])
})

test("`repoctl tsrs check` fails on a release workflow that moved away", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-tsrs-release-"))
  try {
    const workflows = join(repository, "tsrs", ".github", "workflows")
    mkdirSync(workflows, { recursive: true })
    writeFileSync(join(workflows, "release.yml"), workflow(upstreamRows))
    await run(checkTsrsReleaseParity(repository))
    writeFileSync(join(workflows, "release.yml"), workflow(upstreamRows.slice(1)))
    await assert.rejects(
      run(checkTsrsReleaseParity(repository)),
      /differs from _tools\/repoctl\/src\/tsrsRelease\.ts:\n- no longer builds aarch64-apple-darwin/
    )
  } finally {
    rmSync(repository, { recursive: true, force: true })
  }
})
