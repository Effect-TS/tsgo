import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import test from "node:test"
import { formatPatchFailure, PatchApplyError, patchSubmodules } from "../src/submodules.ts"
import { getTypeScriptSource } from "../src/upstream.ts"

const git = (cwd: string, ...args: Array<string>) =>
  execFileSync("git", ["-c", "user.name=test", "-c", "user.email=test@example.com", ...args], {
    cwd,
    encoding: "utf8"
  }).trim()

const patch = (from: string, to: string) => [
  "diff --git a/file.txt b/file.txt",
  "--- a/file.txt",
  "+++ b/file.txt",
  "@@ -1 +1 @@",
  `-${from}`,
  `+${to}`,
  ""
].join("\n")

test("names the patch that no longer applies and reports it to GitHub Actions", async() => {
  const repository = mkdtempSync(join(tmpdir(), "repoctl-patches-"))
  const previousOutput = process.env.GITHUB_OUTPUT
  const output = join(repository, "github-output")
  try {
    const checkout = join(repository, "tsrs")
    mkdirSync(checkout)
    git(checkout, "init", "--quiet")
    writeFileSync(join(checkout, "file.txt"), "one\n")
    git(checkout, "add", "file.txt")
    git(checkout, "commit", "--quiet", "-m", "initial")
    const revision = git(checkout, "rev-parse", "HEAD")
    const patches = join(repository, "_patches", "tsrs")
    mkdirSync(patches, { recursive: true })
    writeFileSync(join(patches, "001-first.patch"), patch("one", "two"))
    writeFileSync(join(patches, "100-second.patch"), patch("one", "three"))
    writeFileSync(output, "")
    process.env.GITHUB_OUTPUT = output

    const error = await Effect.runPromise(
      patchSubmodules(repository, getTypeScriptSource("typescript")).pipe(
        Effect.flip,
        Effect.provide(NodeServices.layer)
      )
    )
    assert.ok(error instanceof PatchApplyError)
    assert.equal(error.checkout, "tsrs")
    assert.equal(error.revision, revision)
    assert.equal(error.patch, "_patches/tsrs/100-second.patch")
    assert.match(error.message, /^_patches\/tsrs\/100-second\.patch does not apply to tsrs [0-9a-f]{40}:\nerror: /)
    assert.equal(readFileSync(join(checkout, "file.txt"), "utf8"), "two\n")

    const outputs = readFileSync(output, "utf8")
    assert.match(outputs, /^patch_failure<<repoctl_patch_failure\n## Patch failure\n/)
    assert.match(outputs, /`_patches\/tsrs\/100-second\.patch` no longer applies to `tsrs` at `[0-9a-f]{40}`/)
    assert.match(outputs, /\npatch_failure_file=_patches\/tsrs\/100-second\.patch\n$/)
  } finally {
    if (previousOutput === undefined) delete process.env.GITHUB_OUTPUT
    else process.env.GITHUB_OUTPUT = previousOutput
    rmSync(repository, { recursive: true, force: true })
  }
})

test("formats a patch failure for the update pull request", () => {
  const revision = "0123456789abcdef0123456789abcdef01234567"
  assert.equal(formatPatchFailure(new PatchApplyError({
    checkout: "tsrs",
    revision,
    patch: "_patches/tsrs/130-checker-rules-visibility.patch",
    detail: "error: patch failed: crates/tsrs_checker/src/lib.rs:12"
  })), [
    "## Patch failure",
    "",
    `\`_patches/tsrs/130-checker-rules-visibility.patch\` no longer applies to \`tsrs\` at \`${revision}\`. ` +
    "Rebase it (and the patches after it) on the new checkout with the `sync-upstream` skill.",
    "",
    "```",
    "error: patch failed: crates/tsrs_checker/src/lib.rs:12",
    "```"
  ].join("\n"))
  assert.equal(formatPatchFailure(new PatchApplyError({
    checkout: "typescript",
    revision,
    patch: "_patches/typescript/001-cmd-tsgo-main.patch",
    detail: ""
  })), [
    "## Patch failure",
    "",
    `\`_patches/typescript/001-cmd-tsgo-main.patch\` no longer applies to \`typescript\` at \`${revision}\`. ` +
    "Rebase it (and the patches after it) on the new checkout."
  ].join("\n"))
})
