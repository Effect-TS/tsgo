---
"@effect/tsgo": minor
---

The `deterministicKeys` rule and the key completions now compute a key for files without a `package.json`, or whose nearest `package.json` has no `name`. The key is built as usual, but without the package name. The sub-directory segment is relative to the `package.json` directory if one is found, otherwise to the `tsconfig.json` directory, otherwise to the program's common source directory.

Projects that enabled `deterministicKeys` without a `package.json` will now see reports. This differs from the upstream `@effect/language-service`, which skips these files.

```json
{
  "name": "@effect/language-service",
  "keyPatterns": [{ "target": "service", "pattern": "default" }]
}
```

```ts
// src/services/AuthService.ts, no package.json, tsconfig.json at the project root
// reports: the expected key is `services/AuthService/MyService`
export class MyService extends Context.Service<MyService, {}>()("MyService") {}
```
