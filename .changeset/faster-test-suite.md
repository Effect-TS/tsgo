---
"@effect/tsgo": patch
---

Speed up the test suite (about 2.3x on a 4-core CI runner) without changing any baseline:

- the test VFS answers lookups of missing paths directly instead of scanning every mounted file, which module resolution does for each extension it probes;
- quick-fix baselines compute each fix's result locally instead of applying and undoing it on the language server, so a test keeps one program and checker;
- tests no longer force a full GC after every program; `repoctl test` bounds each test binary's heap with `GOMEMLIMIT=2GiB` instead;
- the diagnostics runner (Go and Rust) reads the Effect packages from a VFS shared by all tests, layered under each test's own files, instead of copying them into every test's VFS.
