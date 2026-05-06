---
"@effect/tsgo": patch
---

Fix automatic `typescript-go` update workflow compatibility with the latest upstream `tsgo` entrypoint changes.

The workflow now updates to the newer upstream submodule revision, keeps the `cmd/tsgo/main.go` hook patch applying cleanly, and refreshes generated shims and flake metadata accordingly.
