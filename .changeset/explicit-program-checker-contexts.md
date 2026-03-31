---
"@effect/tsgo": minor
---

Refactor internal rules, fixables, refactors, and completions to thread program,
checker, and type parser state explicitly through shared contexts. Simplify the
typescript-go hooks and move completion coverage onto the real fourslash-based
language-service pipeline.
