# TypeScript patches

Store Effect-owned patches against the unified TypeScript revision recorded in
`_packages/tsgo/upstream.json` here. Paths are rooted at the TypeScript checkout,
so Go compiler sources live below `tsc/`. `repoctl submodules setup` applies
`*.patch` files in bytewise filename order when the selected provider is
`microsoft/TypeScript`.

The legacy `microsoft/typescript-go` patch stack remains in
`_patches/typescript-go/` and is selected independently.
