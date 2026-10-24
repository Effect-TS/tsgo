---
name: port-next
description: Orchestrate a round of Rust porting work - pick the Go packages that are ready (all imports ported), hand each to a port-package agent in its own worktree, review and merge the results. The initial port is complete; use this only when a large body of new in-scope Go code (a new package, a big feature) is ported in parallel, or when asked to "continue the port" / "port the next batch".
---

# Run the next porting round

You coordinate; `port-package` agents do the porting.

The initial port is complete (`docs/TSRS_PORT.md`, Phase 4 done): `require_new = true`, there is no baseline
allowlist, and every PR that changes in-scope Go code updates the Rust port in the same PR (`port-change`). A
round is therefore only needed for a large addition ported in parallel, all of it landing with (or right before
merging) the Go change that adds it; `repoctl tsrs port-check` and the baseline runner must pass on `main` at all times.

Each round:

## 1. Choose

1. On an up-to-date `main` (after `pnpm exec repoctl submodules setup --component tsrs`; `pnpm setup-repo` removes `tsrs/`): `pnpm exec repoctl tsrs port-check`, `cargo clippy --all-targets
   -- -D warnings` and `cargo test` must pass. If they don't, fixing that is the round.
2. Read `docs/TSRS_PORT.md`, "Phases". Phase 3 packages (core: `etscore`, `typeparser`, the rule framework,
   `rulerunner`, `etscheckerhooks`) are ported one at a time and reviewed carefully; fan-out (phase 4) starts only
   when they are done.
3. `pnpm exec repoctl tsrs port-ready` lists packages whose imports are all ported, largest first. Choose up to N
   (default 4), preferring packages that unblock the most others, and that nobody else is working on (check open branches and PRs).
4. Split a package with more than about 2k lines to port into ranges of whole Go files
   (`pnpm exec repoctl tsrs port-status <package> --files`). For large flat packages (`internal/rules`,
   `internal/fixables`, `internal/refactors`, `internal/completions`), batch by file: one rule (and its test cases)
   per unit is natural. All ranges write the same crate: the first agent creates the crate and `lib.rs`, the
   others add modules; or create the crate skeleton yourself before dispatching.

## 2. Dispatch

Give each package (or range) to its own agent, in its own git worktree on a branch `port/<crate>[-<n>]` from the
current `main`. Each brief contains:

- the package (and file range), and the instruction to follow the `port-package` skill;
- the round's other assignments, so the agent knows what it must not edit;
- whether it may add `_patches/tsrs` patches or `ets_shim` config (default: no, report instead);
- anything from earlier rounds relevant to it (decisions, reviewer objections).

## 3. Review and merge

For each finished agent:

1. Read its report and its diff. Reject (send back with reasons) work that:
   - leaves a `todo!()` or an unported declaration in its range without a `skipped.tsv` reason;
   - changes the Go control flow, order of operations or message texts;
   - drops or weakens a ported test, changes an expectation without a `docs/DIVERGENCES.md` entry with evidence, or
     leaves a baseline mismatch;
   - depends on a `tsrs_*` crate directly, or edits `tsrs/` / `crates/ets_shim/`;
   - acks declarations it did not port;
   - makes a mapping decision that contradicts `docs/PORTING.md`, or a new one without recording it there.
2. Spot-check at least three non-trivial functions against the Go source, side by side.
3. Rebase onto `main` and rerun `pnpm exec repoctl tsrs port-check`, `cargo clippy --all-targets -- -D warnings` and
   `cargo test` (including the baseline runner) for the whole workspace. Merge only when green. Never force-push
   `main`.

## 4. Report

Give `pnpm exec repoctl tsrs port-status` totals before and after, the baseline summary line of the runner, what was
merged, what was sent back and why, and what `repoctl tsrs port-ready` shows for the next round.
