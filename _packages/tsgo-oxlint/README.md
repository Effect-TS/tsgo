# @effect/tsgo-oxlint

Effect-specific type-aware rules for [Oxlint](https://oxc.rs), for projects that use `@effect/tsgo`.

This package installs the Oxlint binding and the `tsgolint` executable built with the Effect rules for your platform: macOS, Windows, or Linux with glibc, on x64 or arm64. Install it alongside `@effect/tsgo`, at the same version:

```sh
pnpm add --save-dev @effect/tsgo @effect/tsgo-oxlint oxlint oxlint-tsgolint
```

Then run `effect-tsgo patch --oxlint`. See the [Oxlint Setup guide](https://github.com/Effect-TS/tsgo/blob/main/docs/README.md) for the configuration and the presets.
