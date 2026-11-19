---
"@effect/tsgo": patch
---

Fix the intermittent win32-x64 tsrs release build failure "malformed instrumentation profile data: not enough space for another header" / "no profile can be merged". tsrs-test's conformance training runs in many worker processes that all write one `suite-%m.profraw` through LLVM's online profile merging, which on Windows occasionally leaves the file truncated, and `llvm-profdata merge` then rejects the whole merge. The release build now checks every raw profile with `llvm-profdata show` after training and, when one is malformed, runs the training again from fresh profiles (up to 3 attempts) instead of failing the job.
