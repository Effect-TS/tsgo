---
"@effect/tsgo": patch
---

Restore blocking mode on piped standard streams before proxying native tools with `execve`, preventing truncated compiler output on macOS.
