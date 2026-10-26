---
name: port-change
description: Mirror the Go changes of the current branch (vs origin/main) into the Rust port, as the same-PR parity policy requires - find affected markers with `repoctl tsrs port-check`, update the Rust faithfully, fix-lines, ack, run Rust tests and baselines, and report. Use whenever a branch changes ported Go code (internal/..., etscore, ets*hooks, etsdiagnostics, ...), when `pnpm check` fails with port hash mismatches, or before opening a PR that touches Go.
---

# Port the Go changes of this branch to Rust

The port is complete, so every PR that changes in-scope Go code (code or tests) must update the Rust port in the
same PR (`docs/TSRS_PORT.md`, "Parity policy"); `pnpm check` fails otherwise. This skill does that for the current branch. Read `docs/PORTING.md` first if you haven't in this session.

## 1. Find what changed

```sh
git fetch origin
git diff --stat origin/main...HEAD -- '*.go' '_patches/typescript*' testdata/tests
pnpm exec repoctl tsrs port-check            # hash mismatches, unknown/misplaced markers, unacked markers
pnpm exec repoctl tsrs port-diff origin/main HEAD
```

Classify every row:

- **changed** (hash mismatch on a ported declaration): the Rust must change.
- **moved** (same declaration, other file or line): move the Rust item to the mirroring file / fix the line.
- **removed**: delete the Rust item and its tests, and anything only it used.
- **added**: port it, with its marker (`repoctl tsrs port-todo <prefix>` prints them), or add it to
  `porting/skipped.tsv` with a concrete reason. `require_new = true` in `porting/scope.toml`, so `repoctl tsrs
  port-check` fails on any in-scope declaration that is neither.
- **skipped** rows touched by the diff: re-read the reason; it may no longer hold.

Changed `_test.go` declarations show up like code (their Rust ports carry `// go test:` markers and are acked the
same way). Also look at what the manifest does not track:

- changed `_patches/typescript/*.patch`: mirror the change in each `_patches/typescript-<gitHead>/` override folder
  (the same patch backported to that commit: same file names, re-made against that TypeScript, zero fuzz; see
  `_patches/typescript/README.md`, "Per-commit override folders") and, where it maps, in the matching `_patches/tsrs/` patch (same number). `repoctl check`
  fails when an override folder and `_patches/typescript` do not hold the same patch files; check the backport with
  `pnpm exec repoctl submodules setup --component tsrs` and `pnpm exec repoctl test`;
- changed shim config (`_tools/gen_shims/config/...`): mirror it in the `ets_shim` generator config; a change to
  `_tools/gen_shims/providers/typescript/` also goes into the self-contained `providers/typescript-<gitHead>/`
  copies where it applies;
- new or changed cases in `testdata/tests/**` and baselines in `testdata/baselines/reference/**`: the Rust runner
  reads them directly; they must pass on the Rust side too.

## 2. Update the Rust

For each changed declaration:

1. Read the Go diff of that declaration: `git diff origin/main...HEAD -- <file>`.
2. Open the Rust item by its marker (`rg -n '// go: <path>:[0-9]+ <Name>$' crates/`).
3. Apply the same change, faithfully: same control flow, same order, same message texts. Don't re-port from
   scratch unless the Go declaration was rewritten, and don't "improve" neighboring code.
4. If the change needs a tsrs API that `ets_shim` doesn't expose, follow `docs/PORTING.md`, "API gaps: a tsrs patch
   or Rust-only code?".

Then:

```sh
pnpm exec repoctl tsrs port-fix-lines
pnpm exec repoctl tsrs port-ack <path> <Name>...     # only for declarations you updated and verified
pnpm exec repoctl tsrs port-check                    # must pass
```

## 3. Verify

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test -p <each touched crate>
cargo test -p ets_effecttest                    # baselines
```

- `pnpm exec repoctl tsrs lint && pnpm exec repoctl tsrs check && pnpm exec repoctl tsrs test` is what the
  Validate tsrs CI workflow runs, after `repoctl tsrs port-check`; `pnpm check` runs no cargo, only
  `repoctl tsrs port-check`, and the Validate workflow runs neither. The cargo steps need `pnpm exec repoctl submodules setup --component tsrs`
  (`pnpm setup-repo` resets to the Go-only setup and removes `tsrs/`).
- A failing baseline or test that matches the new Go reference is a Rust bug: use the `fix-parity` method.
- Any baseline mismatch fails (there is no allowlist). If the Go change creates a mismatch you cannot fix, stop and
  report it; don't merge the Go change without the Rust.

## 4. Report

- The Go declarations changed / moved / removed / added, and for each the Rust item updated (crate, file).
- `repoctl tsrs port-check` result, Rust test and baseline results.
- Anything not mirrored, and why (a missing tsrs patch, a suspected TypeScript-version difference with its
  evidence).
- Commit the Rust changes on the same branch as the Go change: `git add crates/ porting/ _patches/tsrs/
  _patches/typescript-*/ docs/`
  (only what you changed), message `port: mirror <short Go change summary>`.
