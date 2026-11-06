---
"@effect/tsgo": patch
---

Normalize distributed type parameters when comparing and inferring conditional types in the TypeScript beta and tsrs compilers.

Backport the conditional-relationship correction from microsoft/TypeScript#64307 so valid generic callback and returning-query declarations retain their types without relaxing invalid-assignment or Effect diagnostics.
