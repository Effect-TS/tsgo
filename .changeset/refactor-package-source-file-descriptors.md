---
"@effect/tsgo": patch
---

Refactor typeparser package export matching to reuse shared package source-file descriptors and canonical checker symbol helpers.

This removes repeated node-to-module export matching logic across Effect-related recognizers while preserving existing diagnostics and quick-fix behavior.
