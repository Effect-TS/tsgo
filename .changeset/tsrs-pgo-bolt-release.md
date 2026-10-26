---
"@effect/tsgo": minor
---

Build the Effect-patched tsrs binary exactly like tsrs's own npm release, and ship it for every target tsrs publishes a glibc, macOS or Windows binary for.

- PGO on every target, plus BOLT on Linux, with tsrs's flags, profile settings, C compiler and strip behaviour. The training is tsrs's own (conformance suite, Go fourslash suite, xstate and webpack) plus the Effect-TS/effect monorepo with every Effect rule on, so the rules are optimized like the compiler instead of being laid out as cold code.
- New tsrs targets: `darwin-x64` and `win32-x64`, next to `darwin-arm64`, `linux-x64` and `linux-arm64`.

```sh
effect-tsgo patch --tsrs    # now also on Intel macOS and Windows x64
effect-tsgo tsrs --noEmit
```
