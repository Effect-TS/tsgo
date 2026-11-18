---
"@effect/tsgo": minor
---

Run the Effect rules on Vite+ 1.7.0, which lints with its own `tsrs` instead of `oxlint-tsgolint` (#1062).

- `effect-tsgo setup` writes `effect-tsgo patch --typescript --oxlint --tsrs` for a project whose `vite-plus` is 1.7.0 or later, so that the `tsrs` Vite+ lints with is Effect-patched and plain `vp lint` and `vp check` report the Effect rules:

  ```json
  {
    "scripts": {
      "prepare": "effect-tsgo patch --typescript --oxlint --tsrs"
    }
  }
  ```

- `effect-tsgo vp` and `effect-tsgo oxlint` run the type-aware rules on the Effect-patched build of the installed `tsrs` when `oxlint-tsgolint` is not installed, with no `--tsrs-lint` flag, and set `TSRS_BINARY` to that build, so every `tsrs` started under them through tsrs's launcher, such as `vp exec tsrs`, is the patched one:

  ```sh
  npx effect-tsgo vp check   # Effect rules on Vite+ 1.7.0, without patching node_modules
  ```
