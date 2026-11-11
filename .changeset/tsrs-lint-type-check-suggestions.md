---
"@effect/tsgo": patch
---

Stop `--tsrs-lint ... --type-check` from reporting Effect suggestions as errors (#1025).

Oxlint showed every type-check diagnostic as an error, so suggestion-level Effect diagnostics such as `schemaClassMissingBrand` made the lint fail even though `effect-tsgo tsrs --noEmit` passed. tsrs-lint now forwards only errors and warnings to `--type-check`. This backports oxc-project/tsrs#959 as a tsrs patch until it lands upstream.

```sh
# before: suggestions reported as errors, exit 1
# after: exits 0, like `effect-tsgo tsrs --noEmit`
effect-tsgo --tsrs-lint vp lint --type-aware --type-check
```
