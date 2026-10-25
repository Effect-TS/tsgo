---
"@effect/tsgo": patch
---

Fix `apiStabilityLeak` missing the members of class and interface types with the TypeScript next tag, which now resolves those members lazily. For example, this leak is reported again:

```ts
/** @stability experimental */
export interface PreviewExperimental {
  value: string
}

// `PreviewStable` exposes `PreviewExperimental`, an experimental API.
export interface PreviewStable {
  leaked: PreviewExperimental
}
```
