---
"@effect/tsgo": patch
---

Extend the walker-rule prefilter to call expressions.

A call expression's type is its resolved signature's return type, and both the signature and that return type are already cached from the main check phase. `NodeCouldBeStrictEffect` now consults them for call nodes and skips the expensive flow-analysis re-check when the declared return type conclusively cannot be a strict Effect. Signature-less calls, optional chains, and every inconclusive return type stay conservative, and `promiseInEffectSuccess` no longer computes a location type for calls only to discard it. Emitted diagnostics are unchanged; on a large Effect monorepo build this removes a further ~4.6% of wall time on top of the reference-node prefilter, bringing the total Effect diagnostics overhead versus a pristine tsgo build of the same commit down to ~17%.
