---
"@effect/tsgo": patch
---

Fix `cryptoRandomUUID` and `cryptoRandomUUIDInEffect` diagnostic messages and rule descriptions in Effect v4 to recommend the Effect `Crypto` module instead of `Random`. In Effect v4, `Random` does not provide `randomUUID` and uses non-cryptographic `Math.random`, whereas cryptographic UUID generation is provided by `Crypto.Crypto` (such as `yield* crypto.randomUUIDv4`).
