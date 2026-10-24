# Shim overlays for the TypeScript tsrs ports

Per-commit override of `../typescript/` (see `_patches/typescript/README.md`) for
TypeScript `7.1.0-dev.20260929` (`b85298b6a81f772d080b0455de0ca9d744cd6fd6`), the
commit the pinned tsrs ports, which predates the rooted-path API. When that commit
is set up, this folder is the only shim overlay (it brings its own complete set,
not a copy of `../typescript/`) and the shims are generated through the backport
prefix, whose `compatibility.go` adapters present the canonical (next) shim API
over the older compiler. It goes with
`_patches/typescript-b85298b6a81f772d080b0455de0ca9d744cd6fd6`: rename or delete
both on a tsrs bump that moves the TypeScript (`repoctl check` reports them as
stale otherwise).
