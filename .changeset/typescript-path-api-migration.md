---
"@effect/tsgo": minor
---

Migrate compiler integrations to the current TypeScript path and configuration APIs, and refresh the TypeScript patch stack for the new upstream snapshot. Keep TypeScript 7.0.2 supported through adapters for the legacy typescript-go provider.

For example, plugin option and diagnostic severity resolution now accept rooted source and config file paths directly, while the legacy shims adapt those calls to TypeScript 7.0.2. Preserve Effect options through upstream option cloning and comparison, and fix include patterns for files in filesystem roots.

Keep the Nix compiler source pin, lock file, and vendor hash synchronized with the updated snapshot, and apply Git binary compiler patches during Nix builds. Preserve the original TypeScript 7.0.2 provider shim APIs for tsgolint, with modern API adapters confined to the canonical shim facade.
