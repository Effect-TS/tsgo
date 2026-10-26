---
"@effect/tsgo": patch
---

Match upstream tsrs npm packaging by building release binaries with `noembed` and shipping the exact `.d.ts` library files from the pinned tsrs checkout beside each executable. Packaged tsrs binaries now load those files from disk instead of embedded libraries.
