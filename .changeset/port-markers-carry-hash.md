---
"@effect/tsgo": patch
---

Rust port bookkeeping: each origin marker now carries the hash of the Go declaration it was last synced to, so `porting/manifest.tsv` and `porting/ported.tsv`, which caused constant merge conflicts, are gone. The `repoctl tsrs port-*` commands compute the manifest in memory, `port-check` no longer fails on a stale marker line number, and `port-ack` writes the hash (and current line) into the markers:

```rust
// go: internal/typeparser/effect_type.go:45 TypeParser.EffectType 8a9c52f8a845e081
```
