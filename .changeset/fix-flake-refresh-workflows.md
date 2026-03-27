---
"@effect/tsgo": patch
---

Fix the flake refresh workflows so TypeScript-Go submodule updates also refresh `flake.nix` and `flake.lock`.

This keeps the Nix flake build inputs aligned with the checked-in submodule and generated shim state.
