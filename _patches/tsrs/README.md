# tsrs patches

Store Effect-owned patches against the tsrs revision recorded in
`_packages/tsgo/upstream.json` (`components.tsrs`, selected by `tags.tsrs.latest`)
here. tsrs (`github.com/oxc-project/tsrs`) is the Rust port of TypeScript-Go that
the Rust crates in `crates/` build on. Paths are rooted at the tsrs checkout, so
crate sources live below `crates/`. `repoctl submodules setup` checks tsrs out at
`tsrs/` (a submodule, like `typescript/`) for the TypeScript component and applies
`*.patch` files in bytewise filename order, in place, with `git apply` (no fuzz).

Each patch ports the `_patches/typescript/` patch with the same number and name,
and only as much of it as the Rust port needs so far. The text before the first
`diff --git` line says what maps to what and lists any tsrs-only difference.
Keep the patches minimal, formatted with tsrs's `rustfmt` and free of warnings.

To change a patch, edit the files in `tsrs/` after a setup, then regenerate the
patch with `git -C tsrs diff -- <files>` (keeping the header) and run
`pnpm setup-repo` again: setup resets the checkout before applying the patches,
so edits that are not captured here are lost.

## Per-version override folders

`upstream.json` records more than one tsrs: the latest release (`tags.tsrs.latest`, the
one these patches target and the Rust port follows) and every tsrs a vite-plus runtime
pins (vite-plus ships tsrs since 1.7.0). Each recorded version is built and released.
When these patches do not apply to another recorded commit, that commit gets a
`_patches/tsrs-<gitHead>/` folder: the same patches, file for file, rebased on it,
applied instead of this folder when that commit is set up
(`repoctl submodules setup --component tsrs --version <v>`). Every change here is
mirrored into each override folder. `repoctl check` fails when an override folder's
patch file names differ from these, or when it names no recorded tsrs commit.
