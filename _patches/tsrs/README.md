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
