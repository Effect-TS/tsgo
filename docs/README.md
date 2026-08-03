# Oxlint Setup

Be sure to have installed both Oxlint and the Effect TypeScript-Go integration. The following commands will install the latest versions of both:

```sh
pnpm add --save-dev @effect/tsgo oxlint oxlint-tsgolint
```

Update the scripts section of your `package.json` to include the following:

```json
{
  "scripts": {
    "prepare": "effect-tsgo patch --oxlint",
  }
}

```
This will patch Oxlint to use the Effect TypeScript-Go integration after any package installation. To avoid patching TypeScript, you can use the `--no-typescript` flag: `effect-tsgo patch --no-typescript --oxlint`. This will patch Oxlint to use the Effect TypeScript-Go integration without patching TypeScript.

Run pnpm install to install the dependencies and run the prepare script to patch Oxlint.

```
pnpm install
```

Effect rules require Oxlint's type-aware mode:

```sh
pnpm exec oxlint --type-aware .
```

The installed `oxlint` and `oxlint-tsgolint` versions must match the versions supported by the installed `@effect/tsgo` release. The patch command validates this before changing either integration.

Use the same integration flags when restoring binaries:

```sh
pnpm exec effect-tsgo unpatch --oxlint
pnpm exec effect-tsgo unpatch --no-typescript --oxlint
```

## Known limitations

- All rules require Oxlint's type-aware mode, because they rely on type information to provide accurate diagnostics.
- No fixes and suggestions are provided for effect rules at the moment, but this is planned for a future release.