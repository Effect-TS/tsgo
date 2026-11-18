---
"@effect/tsgo": patch
---

Ship a patched tsrs build for every tsrs version that vite-plus pins. Since vite-plus 1.7.0 the type-aware linter is `tsrs` instead of `oxlint-tsgolint`: vite-plus 1.7.0 depends on `tsrs@7.1.0-beta.tsrs2`, which `@effect/tsgo` now packages next to the latest tsrs (`0.25.0-ts7.1.0-beta`), so a project on vite-plus 1.7.0 gets a patched tsrs matching its installed one.
