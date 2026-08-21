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

## LSP diagnostics

When you have the Effect LSP enabled as well, we recommend setting `diagnostics` to `false` in the LSP plugin settings so that Effect diagnostics are reported only by Oxlint and do not appear twice:

```jsonc
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "diagnostics": false
      }
    ]
  }
}
```

Run pnpm install to install the dependencies and run the prepare script to patch Oxlint.

```
pnpm install
```

Effect rules require Oxlint's type-aware mode and the `effecttsgo` plugin. The recommended preset enables both and configures the recommended Effect rules. Use the schema shipped with `@effect/tsgo` for validation and completions:

```json
{
  "$schema": "./node_modules/@effect/tsgo/oxlint-schema.json",
  "extends": [
    "./node_modules/@effect/tsgo/oxlint-presets/recommended.json"
  ]
}
```

The package also provides presets for each diagnostic category: `correctness`, `antipattern`, `effect-native`, and `style`. Extended configurations are applied in order, and rules in the project configuration take precedence, so categories can be combined and individual rules can be adjusted:

```json
{
  "$schema": "./node_modules/@effect/tsgo/oxlint-schema.json",
  "extends": [
    "./node_modules/@effect/tsgo/oxlint-presets/correctness.json",
    "./node_modules/@effect/tsgo/oxlint-presets/effect-native.json"
  ],
  "rules": {
    "effecttsgo/global-date": "error",
    "effecttsgo/global-console": "off"
  }
}
```

With `oxlint.config.ts`, import the same configurations from the package:

```ts
import { recommended } from "@effect/tsgo/oxlint-presets"
import { defineConfig } from "oxlint"

export default defineConfig({
  extends: [recommended]
})
```

You can then run Oxlint normally:

```sh
pnpm exec oxlint .
```

The installed `oxlint` and `oxlint-tsgolint` versions must match the versions supported by the installed `@effect/tsgo` release. The patch command validates this before changing either integration.

Use the same integration flags when restoring binaries:

```sh
pnpm exec effect-tsgo unpatch --oxlint
pnpm exec effect-tsgo unpatch --no-typescript --oxlint
```

## Known limitations

- All rules require Oxlint's type-aware mode, because they rely on type information to provide accurate diagnostics.
