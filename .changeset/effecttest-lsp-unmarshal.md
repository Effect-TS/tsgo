---
"@effect/tsgo": patch
---

Fix `internal/effecttest` LSP test helpers broken by the `typescript-go` update: the untyped `SendRequestWorker` now returns the response result as a raw `json.Value`, so the inlay hint, diagnostic, and code action helpers decode it via `RequestInfo.UnmarshalResult` instead of type-asserting the typed response struct.
