---
"@effect/tsgo": patch
---

CI: cap the Go build cache saved by the TypeScript validation and release jobs at 2 GB. The cache had grown to about 3.5 GB compressed per TypeScript version, and jobs that restore it were running out of disk. The new `repoctl go-cache trim` command keeps the most recently used files of `go env GOCACHE` up to the size limit and removes the rest:

```sh
repoctl go-cache trim --max-size 2GB
```
