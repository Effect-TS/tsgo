---
name: port-package
description: Port one Go package of effect-tsgo (internal/..., etscore, etscheckerhooks, etslshooks, ...) to its Rust crate under crates/, faithfully, with its tests and origin markers, then ack it. Use when asked to port a package, a file range of a package, or the declarations `repoctl tsrs port-todo` lists for a prefix.
---

# Port a Go package to Rust

The argument is a Go package path (`internal/rules`, `etscore`, `internal/typeparser`), optionally narrowed to
files (`internal/rules: catch_*.go`).

## Before you start

1. Read `porting/PORTING.md` completely, then `porting/TSRS_PORT.md` ("Decisions", "Parity policy"). For a package that
   touches the checker, AST or LS, also read `porting/TSRS_API.md`.
2. `pnpm exec repoctl submodules setup --component tsrs` must have run (the `tsrs/` submodule with `_patches/tsrs` applied and
   `crates/ets_shim` generated; `pnpm setup-repo` resets to the Go-only setup and removes `tsrs/`). Never edit `tsrs/` or `crates/ets_shim/`.
3. `pnpm exec repoctl tsrs port-deps <package>`. Every in-scope import must be ported. If one isn't, stop and report it:
   port the import first, or ask whoever assigned you. Never stub a missing callee.
4. `pnpm exec repoctl tsrs port-crate <package>` gives the crate name.
5. `pnpm exec repoctl tsrs port-todo <package>/` lists what is left, with markers ready to copy. If part of the package is
   already ported, read the existing crate first and follow its choices.

## Port

- Create `crates/<crate>/` if it doesn't exist, from the `Cargo.toml` template in `porting/PORTING.md` ("Layout"), and
  add it to the root `[workspace.dependencies]`. Dependencies are the Go imports, one for one, plus `ets_shim` for
  anything from `shim/...`. Never a `tsrs_*` crate.
- Go file by Go file, in declaration order: for each declaration in `todo`, write the Rust item with its marker
  (`// go: <path>:<line> <Name>`, as `todo` prints it, without a hash: `port-ack` adds it) on the line above. Keep the Go control flow, the order of operations, the message
  texts and the comments that explain why.
- Go `init()` registration (rules, hooks) -> `pub(crate) fn` with the `init` marker, called from the crate's
  `pub fn init()` (`porting/PORTING.md`, "Go runtime equivalents").
- Port the package's `_test.go` files the same way, with `// go test: <path>:<line> <Name>` markers. Port every case
  and keep the expectations exactly. Fixtures: read in place, or copy byte for byte to `crates/<crate>/testdata/`.
- A tsrs API you need is missing or private: follow `porting/PORTING.md`, "API gaps: a tsrs patch or Rust-only code?".
  A new `_patches/tsrs` patch or `ets_shim` config change is a foundations change: make it only if your brief allows
  it, otherwise report it.
- A mapping `porting/PORTING.md` doesn't cover: choose the most Go-faithful one, apply it consistently, and add it to
  `porting/PORTING.md` (or `porting/TSRS_API.md`) in the same commit.
- A declaration that shouldn't be ported goes in `porting/skipped.tsv` with a reason that fits "What not to port".

## Check

Run all of these and fix what they find in your own code:

```sh
cargo fmt -p <crate>
cargo clippy -p <crate> --all-targets -- -D warnings
cargo test -p <crate>
cargo test -p ets_effecttest                 # baselines; or filter to the cases your package affects
pnpm exec repoctl tsrs port-check
pnpm exec repoctl tsrs port-status <package>
```

- A failing ported test or baseline means the Rust is wrong until shown otherwise; use the `fix-parity` skill.
  Change an expectation only as `porting/DIVERGENCES.md` allows, and add the entry.
- Every baseline must match: there is no allowlist, a mismatch fails the runner (as in Go).
- Never `#[allow]`; use `#[expect(clippy::<lint>, reason = "...")]` on the smallest item.

## Ack

When a declaration's Rust is complete and its tests pass:

```sh
pnpm exec repoctl tsrs port-ack <path> <Name>...        # or: pnpm exec repoctl tsrs port-ack --prefix <package>/
                                                        # (writes the Go hash at the end of each marker)
pnpm exec repoctl tsrs port-check                       # must pass
```

Ack only what you actually ported and verified.

## Finish

- Commit only your paths: `git add crates/<crate> Cargo.toml Cargo.lock porting/` (only what you changed),
  message `<crate>: port <what> (<n>/<m> decls)`.
- Report: the `repoctl tsrs port-status <package>` line, what isn't ported and why, divergences recorded, the baseline
  summary line, anything another crate or the foundations (tsrs patches, `ets_shim`) need, and
  decisions you added to the docs.
