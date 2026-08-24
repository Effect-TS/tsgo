# TypeScript Go repository migration

Status: research and implementation validation as of 2026-08-24. This note distinguishes confirmed upstream facts from recommendations for this repository.

## Conclusion

TypeScript's Go compiler did not merely move repositories. Its Go module identity changed from `github.com/microsoft/typescript-go` to `github.com/microsoft/TypeScript/tsc`, and the compiler moved from the repository root to the `tsc/` subdirectory. TypeScript 7.0.2's npm `gitHead` points to the old repository/root layout and uses the old module identity. Microsoft `main` and the recreated `ts7-release` branch use the new repository, nested layout, and module identity. [Microsoft's migration PR](https://github.com/microsoft/TypeScript/pull/63763) explicitly calls the nested `tsc/` directory the main, currently unversioned Go module; the old [7.0.2 `go.mod`](https://github.com/microsoft/typescript-go/blob/typescript/v7.0.2/go.mod) and current [new `tsc/go.mod`](https://github.com/microsoft/TypeScript/blob/main/tsc/go.mod) show the two module declarations.

The practical recommendation is to model these as two upstream layouts, not as one repository whose URL happened to change:

| Track | Conservative source / checkout | Go module root | Go module prefix | Patch stack |
| --- | --- | --- | --- | --- |
| TypeScript 7.0.2 | `microsoft/typescript-go` / `typescript-go` | `.` | `github.com/microsoft/typescript-go` | `_patches/typescript-go/` |
| Migrated releases and development | `microsoft/TypeScript` / `typescript` | `tsc/` | `github.com/microsoft/TypeScript/tsc` | `_patches/typescript/` |

Keep the old upstream source checkout separate from `_patches`; that directory should contain patch artifacts, not a vendored compiler repository. Keep the 7.0.2 patch stack frozen and rebase a distinct new stack against `TypeScript/tsc`.

Because this repository's Effect source must build against both tracks without changing its imports, use the migrated `github.com/microsoft/TypeScript/tsc/shim/...` namespace as the invariant application-facing path. Modern builds generate that namespace directly in `shim/`. Legacy builds generate the provider-native `github.com/microsoft/typescript-go/shim/...` modules in `shim/` and a second compatibility layer in `shim/_backport/`; the compatibility packages re-export the old provider shims and never import old `internal` packages directly.

This direction is now implemented. `repoctl` selects the checkout, module root, patch stack, shim prefix, and provider-specific compatibility inputs from recorded upstream metadata. The modern `next` pin and legacy 7.0.2 pin both pass `pnpm check` with the same Effect imports. The released legacy oxlint-tsgolint component also passes in the combined workspace, including its own shim overlays.

## What Microsoft changed

Microsoft had already announced that the `tsgo` name was effectively gone and that development would return to `microsoft/TypeScript` for a unified issue backlog. [typescript-go discussion #4576](https://github.com/microsoft/typescript-go/discussions/4576)

The migration was merged into `microsoft/TypeScript:main` on 2026-08-20. The PR says the complete `typescript-go` history was replayed, the code was placed in `tsc/` to avoid collisions with existing TypeScript tags, and the nested module remains unversioned and does not yet expose an API. [Microsoft TypeScript PR #63763](https://github.com/microsoft/TypeScript/pull/63763)

Two mechanical commits make the compatibility boundary concrete:

- [`28c68f4`](https://github.com/microsoft/TypeScript/commit/28c68f49f846413d6c356394cc8cb2015e3aa264) changes `module github.com/microsoft/typescript-go` to `module github.com/microsoft/TypeScript/tsc` and rewrites compiler imports to that prefix.
- [`5f647a8`](https://github.com/microsoft/TypeScript/commit/5f647a841a6a4dec9d8594854c320d5808134809) applies the combined TypeScript 7 repository layout. The root [Go workspace](https://github.com/microsoft/TypeScript/blob/main/go.work) now includes `./tsc`.

The stable 7.0.2 tag is commit [`2bd066d`](https://github.com/microsoft/typescript-go/tree/typescript/v7.0.2). Its module is still `github.com/microsoft/typescript-go`, with compiler directories such as `internal/ast` at the repository root. [7.0.2 `go.mod`](https://github.com/microsoft/typescript-go/blob/typescript/v7.0.2/go.mod)

Microsoft also replayed the old release tags into the unified repository. The [`microsoft/TypeScript` `v7.0.2` tag](https://github.com/microsoft/TypeScript/tree/v7.0.2/tsc) contains the compiler under `tsc/`, but its `tsc/go.mod` still declares the old `github.com/microsoft/typescript-go` module. Its `tsc` subtree has Git tree `ed2c2c12c401b84bd5888d0b889737495aa93a20`, exactly the same tree as `typescript-go` commit `2bd066d`. [TypeScript tree object](https://api.github.com/repos/microsoft/TypeScript/git/trees/ed2c2c12c401b84bd5888d0b889737495aa93a20), [typescript-go tree object](https://api.github.com/repos/microsoft/typescript-go/git/trees/ed2c2c12c401b84bd5888d0b889737495aa93a20)

This gives a later normalization option: one `microsoft/TypeScript` checkout can supply both `v7.0.2/tsc` and newer `tsc/` revisions. It does **not** normalize the Go namespace, and npm's 7.0.2 `gitHead` still names the old-repository commit. The conservative first step is therefore to preserve the existing 7.0.2 provider while making source layout explicit; checkout unification can follow once `repoctl` has a reliable old-SHA-to-replayed-tag mapping.

Therefore old and new imports are distinct Go package identities even when declarations are otherwise source-equivalent. The new line is not source-compatible merely by changing a checkout URL.

## Can one shim redirect between the namespaces?

The shim technique remains viable, but the low-level shim module path has to match the selected compiler namespace:

- 7.0.2 shims: `github.com/microsoft/typescript-go/shim/...`
- migrated shims: `github.com/microsoft/TypeScript/tsc/shim/...`

This constraint comes from Go's `internal` visibility rule: a package below an `internal` directory can only be imported by code beneath the import-path tree rooted at the parent of `internal`. [Official Go command documentation](https://pkg.go.dev/cmd/go#hdr-Internal_Directories) A neutral package such as `github.com/effect-ts/tsgo/shim/ast` therefore cannot directly import either upstream's `internal/ast`, and an old Microsoft-prefixed shim cannot import the new Microsoft-prefixed internal package.

A `replace` directive changes where module content is found; it does not turn the old and new import paths into the same package identity or bypass `internal` visibility. [Official `go.mod` reference](https://go.dev/doc/modules/gomod-ref#replace) It also cannot safely conceal symbol-path changes used by any `go:linkname` bridge.

There are two workable designs:

1. Select one provider during setup and generate/import that provider's shim namespace. This is the simpler model and is what Oxc's migration currently demonstrates.
2. If unchanged Effect source imports truly must compile against either provider, make the migrated shim namespace canonical. Generate provider-native shims in `shim/`; for the legacy provider only, generate canonical backports in `shim/_backport/` that re-export the old shim modules. The provider shim remains under the Microsoft prefix required to access `internal`, while the application always imports the migrated namespace.

The second design is the implemented design here. It doubles generated shim surface area only while using the legacy provider; modern builds contain one provider-native layer. When 7.0.2 support is dropped, `_backport` generation and the legacy provider descriptor can be removed without changing Effect imports.

## Patch implications

The patch stacks are necessarily layout-specific. Old patches address paths such as `internal/project/project.go` and contain the old import prefix. New patches address `tsc/internal/project/project.go`, may contain the new prefix, and must account for upstream code drift. A path-prefix option alone is insufficient for hunks that mention imports or changed code.

Use separate `_patches/typescript-go/` and `_patches/typescript/` series, with a common logical patch name where the intent is shared. Do not try to apply one physical patch file verbatim to both repositories. The old 7.0.2 source and patches can then remain stable while the new series follows `ts7-release` or `main`.

### Patch application experiment (2026-08-24)

The current 24-file Effect patch series was tested sequentially in disposable checkouts of [`ts7-release` at `b99c439d`](https://github.com/microsoft/TypeScript/tree/b99c439debe1f6a8765e877c3753620f8afb4f3e) and [`main` at `e9e47745`](https://github.com/microsoft/TypeScript/tree/e9e477458d7b975ed5e43117fb85f6bc87363a6a):

| Application form | `ts7-release` | `main` |
| --- | ---: | ---: |
| Existing patches verbatim, from the repository root | 0/24 | 0/24 |
| Add only the `tsc/` path prefix | 17/24 | 12/24 |
| Add `tsc/`, rewrite `github.com/microsoft/typescript-go` to `github.com/microsoft/TypeScript/tsc`, and rename `cmd/tsgo` to `cmd/tsc` | 24/24 | 14/24 |

The verbatim failures are all caused first by the repository layout: paths such as `internal/checker/checker.go` do not exist at the unified repository root. On `ts7-release`, the remaining seven failures are mechanical rather than semantic: patch 001 names the renamed `cmd/tsgo/main.go`, and patches 001, 007, 009, 013, 015, 024, and 028 contain the old module prefix as hunk context or, in patch 009, as a newly added import. After those three rewrites, the complete series applies cleanly, producing 27 changed files with 809 insertions and 33 deletions, and `git diff --check` passes. This shows that the release branch still has the source shape expected by the current Effect patches, but the patch artifacts themselves are not reusable verbatim.

`main` has genuine contextual and behavioral drift beyond those mechanical changes. Patches 007, 011, 012, 013, 015, 018, 022, 024, 027, and 028 fail. Examples include internal compiler-option tags changing, content-mapping support altering completion/hover/inlay-hint/document-symbol flows, auto-import state gaining canonical source-file paths, and incremental build info gaining content-mapper identities. The current source at the tested commit makes those changed seams visible in [`compileroptions.go`](https://github.com/microsoft/TypeScript/blob/e9e477458d7b975ed5e43117fb85f6bc87363a6a/tsc/internal/core/compileroptions.go), [`completions.go`](https://github.com/microsoft/TypeScript/blob/e9e477458d7b975ed5e43117fb85f6bc87363a6a/tsc/internal/ls/completions.go), [`hover.go`](https://github.com/microsoft/TypeScript/blob/e9e477458d7b975ed5e43117fb85f6bc87363a6a/tsc/internal/ls/hover.go), [`symbols.go`](https://github.com/microsoft/TypeScript/blob/e9e477458d7b975ed5e43117fb85f6bc87363a6a/tsc/internal/ls/symbols.go), [`autoimport/view.go`](https://github.com/microsoft/TypeScript/blob/e9e477458d7b975ed5e43117fb85f6bc87363a6a/tsc/internal/ls/autoimport/view.go), and [`incremental/buildInfo.go`](https://github.com/microsoft/TypeScript/blob/e9e477458d7b975ed5e43117fb85f6bc87363a6a/tsc/internal/execute/incremental/buildInfo.go). Those ten patches need a real rebase and, for hooks that now operate over projected files, a semantic review rather than a context refresh.

The combined patch order must also be treated as part of the provider profile: current `repoctl` applies tsgolint's upstream patches before the Effect series. The refreshed five-patch series in [tsgolint PR #1143](https://github.com/oxc-project/tsgolint/pull/1143) already uses repository-root `tsc/...` paths and the new module prefix. At PR head [`2cba331d`](https://github.com/oxc-project/tsgolint/tree/2cba331dbcd7170d196399443fab1bec42e83b68/patches), it pins the TypeScript migration merge commit [`6d44e058`](https://github.com/microsoft/TypeScript/commit/6d44e0584a857f3a03794241197fd9c7ff457499), not `ts7-release`. All five patches apply to the tested current `main`, but patch 0002 fails against `ts7-release` in four `internal/project` files. Thus neither combined modern profile is ready solely by selecting a folder: the `ts7-release` profile needs Oxc's patches rebased, while the `main` profile needs ten Effect patches rebased.

The recommendation remains two physical patch directories selected by the recorded provider/revision. The new `_patches/typescript/` stack can initially be generated from the mechanical release-branch rewrite, but it should be committed as an independent series and validated together with the exact tsgolint patch series and TypeScript SHA. This preserves the frozen 7.0.2 stack and makes future `main` drift explicit instead of hiding transformation rules inside setup code.

## What oxlint/tsgolint is doing

Oxc first stopped automatically advancing its old compiler submodule in merged [tsgolint PR #1116](https://github.com/oxc-project/tsgolint/pull/1116). Its released/main line therefore remains on the stable 7.0.2-era arrangement while the repository transition is resolved.

Open [tsgolint PR #1143](https://github.com/oxc-project/tsgolint/pull/1143) implements the migration rather than a dual-provider abstraction. It:

- changes the submodule from `microsoft/typescript-go` at `typescript-go/` to a shallow `microsoft/TypeScript` checkout at `typescript/`;
- changes `go.work` from `./typescript-go` to `./typescript/tsc`;
- changes application imports, `require`/`replace` entries, and generated shim module identities to `github.com/microsoft/TypeScript/tsc/...`;
- refreshes all five upstream patches, including changing patch paths from `internal/...` to `tsc/internal/...`;
- applies patches from the TypeScript repository root and then pins/regenerates shims against `github.com/microsoft/TypeScript/tsc@$COMMIT`.

The implementation's substantive GitHub Actions jobs are green: Go tests, end-to-end tests, lint, typo checking, schema checking, autofix, and security analysis passed; the conditional Windows job was skipped. [Main CI run](https://github.com/oxc-project/tsgolint/actions/runs/32354013278), [schema run](https://github.com/oxc-project/tsgolint/actions/runs/32354013284), [autofix run](https://github.com/oxc-project/tsgolint/actions/runs/32354013273), [security run](https://github.com/oxc-project/tsgolint/actions/runs/32354013268)

The PR is deliberately still open. In its maintainer discussion, TypeScript author Jake Bailey recommends pointing at the recreated `ts7-release` branch and says a 7.0.3 tag will follow; tsgolint maintainer Cameron Clark says they will probably wait for that patch release so the target is unambiguous. [Jake Bailey's comment](https://github.com/oxc-project/tsgolint/pull/1143#issuecomment-5354054603), [Cameron Clark's reply](https://github.com/oxc-project/tsgolint/pull/1143#issuecomment-5356334451) This indicates a staged strategy: keep 7.0.2 stable now, land the already-working repository/module migration around 7.0.3, and do not attempt simultaneous provider support inside tsgolint.

## TypeScript 7.0.3 timing (checked 2026-08-22)

**Explicit schedule:** Microsoft has not published a date or release window. The only direct timing statement found is Jake Bailey's “Eventually we'll do a 7.0.3 and tag” in the [tsgolint discussion](https://github.com/oxc-project/tsgolint/pull/1143#issuecomment-5354054603). Oxc's decision to wait for the next patch release is Oxc's dependency decision, not a Microsoft delivery commitment.

**Concrete preparation:** the recreated `ts7-release` branch is already configured as a stable TypeScript release with `nativePreviewReleaseVersion = "7.0.3"`, and the compiler's internal version is also `7.0.3`. [`Herebyfile.mjs` at the branch head](https://github.com/microsoft/TypeScript/blob/b99c439debe1f6a8765e877c3753620f8afb4f3e/Herebyfile.mjs#L88-L96), [`version.go` at the branch head](https://github.com/microsoft/TypeScript/blob/b99c439debe1f6a8765e877c3753620f8afb4f3e/tsc/internal/core/version.go#L7-L9) Microsoft's earlier release-tooling PR says these are the switches needed to release 7.0.3, but gives no schedule. [typescript-go PR #4456](https://github.com/microsoft/typescript-go/pull/4456)

The branch's 2026-08-20 CI run successfully completed its build and release-build jobs, along with the ordinary platform tests and lint jobs; the overall workflow was red only because the race-mode test failed. [CI run](https://github.com/microsoft/TypeScript/actions/runs/32332378036), [successful release-build job](https://github.com/microsoft/TypeScript/actions/runs/32332378036/job/96315361582), [failed race-mode job](https://github.com/microsoft/TypeScript/actions/runs/32332378036/job/96315361708) This is a readiness signal, not a release-date signal. The repository's older release/version workflows are currently under [`disabled-workflows`](https://github.com/microsoft/TypeScript/tree/ts7-release/.github/disabled-workflows), so the public GitHub Actions configuration does not expose a scheduled publication date.

**Public release state:** npm still tags `7.0.2` as `latest`, the registry has no `typescript@7.0.3`, GitHub's latest stable release/tag is `v7.0.2`, and the public milestone list contains TypeScript 7.1 but no 7.0.3 milestone. [npm dist-tags](https://registry.npmjs.org/-/package/typescript/dist-tags), [missing npm version](https://registry.npmjs.org/typescript/7.0.3), [Microsoft releases](https://github.com/microsoft/TypeScript/releases), [Microsoft milestone API](https://api.github.com/repos/microsoft/TypeScript/milestones?state=all&per_page=100)

Confidence is **high** that 7.0.3 is intended and mechanically prepared, and **high** that no public date or window exists as of the check date. Calling it “imminent,” or assigning even an approximate week, would be inference unsupported by the public primary sources.

## `repoctl update` changes implied by the migration

Before this migration work, the updater recorded only a TypeScript version and Git SHA, assumed `microsoft/typescript-go` for comparisons, and assumed the `typescript-go` gitlink when resolving a tsgolint release. See [`_tools/repoctl/src/upstream.ts`](../../_tools/repoctl/src/upstream.ts) and [`upstream.json`](../../_packages/tsgo/upstream.json) for the implemented provider-aware model.

This is already a live failure, not only a predicted one. [Effect-TS/tsgo's 2026-08-22 Update Upstreams run](https://github.com/Effect-TS/tsgo/actions/runs/32563825883) failed after npm moved `typescript@next` from old-repository commit `1bcfa18...` to new TypeScript commit `d6c4afd...`: `repoctl` sent both endpoints to the hard-coded `microsoft/typescript-go/compare` API, which returned 404.

Make compiler source layout explicit metadata, approximately:

```ts
interface TypeScriptSource {
  repository: "microsoft/typescript-go" | "microsoft/TypeScript"
  checkoutDir: "typescript-go" | "typescript"
  moduleDir: "." | "tsc"
  modulePrefix: "github.com/microsoft/typescript-go" | "github.com/microsoft/TypeScript/tsc"
  providerShimPrefix: "github.com/microsoft/typescript-go/shim" | "github.com/microsoft/TypeScript/tsc/shim"
  patchDir: "_patches/typescript-go" | "_patches/typescript"
  shimOverlayDir: string
  gitlinkPath: "typescript-go" | "typescript"
  revision: string
}
```

Then use that metadata for all of the following:

- checkout/submodule URL and directory;
- workspace module directory;
- patch selection and application root;
- shim generator internal prefix, shim module prefix, `require`, and `replace` entries;
- tsgolint gitlink lookup (`typescript-go` for existing releases, `typescript` for migrated releases);
- GitHub commit, compare, and PR-description links.

The old and new Git SHAs belong to different repositories, and the replayed migration rewrote history. A cross-boundary GitHub compare cannot be expressed as the current `microsoft/typescript-go/compare/<old>...<new>` call. At the transition, report the old and new commits separately and link the migration PR; resume ordinary comparisons once both endpoints use `microsoft/TypeScript`.

NPM metadata should remain the source for selecting TypeScript versions, but `gitHead` alone is no longer enough to locate source. During updates, probe the exact `gitHead` in the two allowlisted upstream repositories, require an unambiguous match, and persist the resulting source-layout discriminator in `upstream.json`. Setup and release builds should consume that recorded value rather than infer a repository from a version-number boundary.

## Implemented rollout

1. The 7.0.2 provider and `_patches/typescript-go/` series remain available as the `latest` channel.
2. Upstream schema version 5 records the provider for every TypeScript version; `repoctl` resolves all checkout and module coordinates through provider descriptors.
3. The `next` channel uses `microsoft/TypeScript` at `typescript/`, module root `typescript/tsc`, and the independent `_patches/typescript/` series.
4. Effect source imports only `github.com/microsoft/TypeScript/tsc/shim/...`; modern builds generate it directly and legacy builds resolve it through generated `_backport` modules.
5. Provider-specific handwritten compatibility helpers hide API drift such as moved types and changed function return/signature shapes.
6. Both standalone profiles passed setup and `pnpm check`; the released 7.0.2 oxlint-tsgolint profile also passed setup, code generation, and the combined workspace check.
