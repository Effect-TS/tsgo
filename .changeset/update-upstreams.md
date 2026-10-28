---
"@effect/tsgo": patch
---

Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261028.1), which ships [`TypeScript`](https://github.com/microsoft/TypeScript/commit/97aedd3b75e8e18a3cf2786ffbdb2e6c88aea23f) commit `97aedd3b75e8e18a3cf2786ffbdb2e6c88aea23f`, and the Rust port to [`tsrs@0.20.1-ts7.1.0-beta`](https://www.npmjs.com/package/tsrs/v/0.20.1-ts7.1.0-beta). Symbol tables are read through per-provider helpers, since TypeScript next replaces the map-based `SymbolTable` with an insertion-ordered one.
