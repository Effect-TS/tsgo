---
"@effect/tsgo": minor
---

Run Oxlint's type-aware rules, including the Effect rules, on tsrs with `effect-tsgo --tsrs-lint`.

tsrs 0.21.0 ships its port of tsgolint as `tsrs --lint`. The Effect-patched tsrs now registers the Effect rules there,
and `--tsrs-lint[=<version|tag>]` in front of `effect-tsgo oxlint` or `effect-tsgo vp` makes the patched Oxlint run
them on it instead of the patched tsgolint:

```sh
effect-tsgo --tsrs-lint oxlint --type-aware   # the installed tsrs version when it ships, else the latest
effect-tsgo --tsrs-lint=latest vp lint        # or a tsrs version or tag
```

The packaged tsrs is updated to 0.21.0.
