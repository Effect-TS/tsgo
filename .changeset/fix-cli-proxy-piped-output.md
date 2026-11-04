---
"@effect/tsgo": patch
---

Fix truncated output from tools run through the CLI proxy (`effect-tsgo tsc`, `tsrs`, `oxlint`, `vp`) when their stdio is a pipe. The proxy built `NodeServices.layer` to resolve the tool, and its `NodeTerminal` read `process.stdin` and `process.stdout`, which leaves piped stdio non-blocking; `execve` handed that to the tool, whose writes then failed once the pipe filled, sometimes with exit code 0. For example, `effect-tsgo tsrs --listFilesOnly` captured by another process stopped after 64 KiB. The proxy now provides only the `FileSystem` and `Path` services it needs.
