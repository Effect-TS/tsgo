# TypeScript patches

Store Effect-owned patches against the unified TypeScript revision recorded in
`_packages/tsgo/upstream.json` here. Paths are rooted at the TypeScript checkout,
so Go compiler sources live below `tsc/`. `repoctl submodules setup` applies
`*.patch` files in bytewise filename order when the selected provider is
`microsoft/TypeScript`.

Additional shim helpers live in
`_tools/gen_shims/providers/typescript/<package>/integration.go`.

The legacy `microsoft/typescript-go` patch stack remains in
`_patches/typescript-go/` and is selected independently.

## Per-commit override folders

This folder and `_tools/gen_shims/providers/typescript/` target the newest
TypeScript (next). A TypeScript component of `upstream.json` whose sources they
no longer fit gets folders named after its `gitHead`, each of which replaces the
plain folder as a whole when that commit is set up (`repoctl submodules setup`,
`getTypeScriptComponentSource` in `_tools/repoctl/src/upstream.ts`):

- `_patches/typescript-<gitHead>/` is applied instead of this folder. It holds
  the same patches, file for file, backported to that commit;
- `_tools/gen_shims/providers/typescript-<gitHead>/` is the only shim overlay
  instead of `providers/typescript/`, so it is self-contained, and the shims are
  generated through the backport prefix
  (`github.com/microsoft/TypeScript/tsc/providershim`), whose
  `compatibility.go` adapters present the canonical (next) shim API over that
  older compiler.

A commit can have either folder or both. The same convention applies to the
legacy provider (`_patches/typescript-go-<gitHead>/`,
`_tools/gen_shims/providers/typescript-go-<gitHead>/`); only a full 40-hex
suffix makes an override, so `_patches/typescript-go` is the legacy provider's
own folder. Today:

- `typescript-b85298b6a81f772d080b0455de0ca9d744cd6fd6` (patches and shims): the
  TypeScript the pinned tsrs ports (`7.1.0-dev.20260929`);
- `typescript-3e9236361fa97a9e6c685f4833ac457ae2d8c50a` (shims only):
  `7.1.0-beta`.

Rules, enforced by `repoctl check` where they can be:

- **Mirroring.** Any change to a patch here (added, changed, renamed or removed)
  is mirrored into every `_patches/typescript-<gitHead>/` in the same pull
  request, and into `_patches/tsrs/` where the patch has a tsrs counterpart.
  `repoctl check` fails when an override folder does not hold the same patch
  file names as this one. Likewise, a change to
  `_tools/gen_shims/providers/typescript/` is mirrored into the self-contained
  shim override copies where it applies (the beta one is a copy of it plus its
  own `tsoptions/` adapters; the tsrs one has its own set). Shim folders are not
  compared by name.
- **Stale folders.** An override folder must name the `gitHead` of a TypeScript
  component with that provider; `repoctl check` fails otherwise ("stale override
  folder"). When beta or the tsrs TypeScript moves to a new commit, rename the
  folder to the new `gitHead` (re-made against it if needed), or delete it when
  the plain folder applies to the new commit.
