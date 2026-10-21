---
"@effect/tsgo": minor
---

Track the `typescript@beta` dist-tag alongside `typescript@latest` and `typescript@next`. `repoctl upstream update` now resolves the beta tag into `upstream.json`, so the TypeScript beta compiler (currently `7.1.0-beta`) is built, tested in CI under the `beta` channel, and shipped with the platform packages.
