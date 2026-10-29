---
"@effect/tsgo": patch
---

Remove the unused execution flow graph parser (`TypeParser.ExecutionFlow`) from the Go build and the Rust port. No rule, refactor or language service feature used it. The test runner no longer writes the `.flows.txt` and `.flows.*.mermaid` baselines. Diagnostics and other user-facing behavior do not change.
