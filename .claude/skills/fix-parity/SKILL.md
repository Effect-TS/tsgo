---
name: fix-parity
description: Find and fix a behavior difference between the Rust port and the Go build - a failing Rust baseline, a failing ported Go test, or a reported Rust-only bug. Use when the Rust baseline runner, a ported test, or a user shows Rust and Go disagree.
---

# Fix a parity difference

The Go code at the current commit is the specification (`docs/PORTING.md`, "The fidelity rule"). A difference is a
Rust bug until evidence shows otherwise.

## Reproduce

- Work in the tsrs setup (`pnpm exec repoctl submodules setup --component tsrs`: the TypeScript next compiler and
  tsrs, both patched, so the Go and the Rust side both build; `pnpm setup-repo` removes `tsrs/`).
- Find the smallest failing unit: one baseline case (`cargo test -p ets_effecttest -- <case>`, which prints the diff
  against `testdata/baselines/reference/...`), or one ported test (`cargo test -p <crate> <test>`).
- If no existing case shows it, add the smallest `.ts` case to `testdata/tests/<suite>/` and generate its reference
  baseline with the Go runner (`go test ./internal/effecttest/ -run <Test>`: for a case with no reference yet it
  writes both `testdata/baselines/local/` and `testdata/baselines/reference/`). Review and commit the new reference.
  The Go runner owns the reference; never write it from Rust.
- Run the case twice on each side. If either side is unstable, it is non-determinism first (tsrs#725: run tsrs with
  `--checkerAssignment go`), not a port bug.
- A difference from your expectations, or from the TypeScript language service plugin, doesn't count. Only the Go
  build does.

## Diagnose

1. Find the first point where they diverge: narrow down with focused tests or temporary instrumentation on both
   sides (remove it afterwards).
2. Locate the Rust item, then its Go original through the marker (`// go: <path>:<line> <Name>`). Compare the whole
   Go function with the Rust one, line by line. Don't reason from memory or from what the code "should" do.
3. Common causes: map iteration order, sort tie order (Go's pdqsort vs Rust's), nil vs empty, integer width,
   position arithmetic (`u32` vs Go `int`), UTF-8 vs UTF-16 offsets, `%v` formatting, diagnostic order, a tsrs
   accessor that returns a non-nil default where Go returns nil (`TSRS_API.md`), a `CheckerHandle` held too long,
   and TypeScript-version differences.
4. Decide which kind it is:
   - **Port bug**: fix it (below).
   - **TypeScript version**: prove it with the Go build at tsrs's TypeScript version (`docs/TSRS_PORT.md`,
     "Decisions"): if Go there produces the Rust output, add a `typescript` entry to `docs/DIVERGENCES.md` with that
     evidence.
   - **tsrs bug** (tsrs differs from tsgo at tsrs's own TypeScript pin): reduce it, add a `tsrs` entry, and either
     work around it (marked `// tsrs workaround: <issue>`) or fix it with a minimal `_patches/tsrs` patch numbered
     100+, with a regression test (`docs/PORTING.md`, "TypeScript: tsrs").
   - **Missing tsrs patch**: the Go side relies on a `_patches/typescript` patch that has no tsrs counterpart yet:
     port that patch (same number), per `docs/PORTING.md`.

## Fix

- Make the Rust do what the Go does: same decomposition, same order. Don't special-case the input, and don't
  "improve" anything. Never change the Go to match the Rust unless the Go is genuinely wrong (then it is a Go fix
  with its own baseline changes, reviewed as such).
- Fix the root cause in the shared helper, then grep the crate and its siblings for the same slip.
- Keep or add a regression case: the baseline case, or a Rust test with the Go output as the expectation.

## Verify and report

Run the crate's tests, the baseline runner, `cargo clippy --all-targets -- -D warnings` and
`pnpm exec repoctl tsrs port-check`. Report the difference, the Go function followed, the root cause, other places with
the same slip, and the evidence. Don't claim success while a check fails.
