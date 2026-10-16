---
"@effect/tsgo": patch
---

Make `apiStabilityLeak` allocate less. The rule used to create an empty findings map for every type, signature and symbol it inspected, and to copy the stored findings into a new map every time it re-checked an unresolved result. Most results never get a finding, and most re-checks add nothing. A result's map is now created when it gets its first finding, and a re-check that adds nothing keeps the stored map instead of copying it.

For example, on the Effect monorepo the rule allocated 605 MB in 5.3 M objects before and 496 MB in 3.4 M objects after. Its CPU went from 3.4 s to 3.2 s.

Reported diagnostics are unchanged.
