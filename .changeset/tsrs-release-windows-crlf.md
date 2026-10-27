---
"@effect/tsgo": patch
---

Fix the win32-x64 tsrs release build. Git on the Windows runner checked out `tsrs/Cargo.toml` with CRLF line endings, so repoctl could not read the `[workspace.metadata.typescript]` commit and the build failed with `No [workspace.metadata.typescript] commit in tsrs/Cargo.toml`. The parser now accepts CRLF, and the Windows release jobs check out with `core.autocrlf` turned off.
