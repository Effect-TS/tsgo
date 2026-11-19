---
"@effect/tsgo": patch
---

Fix the tsrs release build failing to fetch BOLT: LLVM's release tarballs are compressed with a 1 GiB zstd window, larger than zstd's default 128 MiB decode limit, so `tar --zstd` aborted with "Frame requires too much memory for decoding". The archive is now decompressed with `zstd -d --long=30`, as tsrs's own release workflow does.
