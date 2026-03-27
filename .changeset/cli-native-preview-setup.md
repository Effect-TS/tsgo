---
"@effect/tsgo": patch
---

Update the setup CLI to detect existing `@typescript/native-preview` dependencies and preserve whether they are installed in `dependencies` or `devDependencies`.

When enabling the language service, the setup flow now also adds `@typescript/native-preview@latest` if it is missing.
