---
"@effect/tsgo": minor
---

Allow `namespaceImportPackages`, `barrelImportPackages`, `importAliases`, and `topLevelNamedReexports` in per-file `overrides`. Auto-import, the import quick fixes, and the `preferNamespaceImport` and `preferBarrelImport` diagnostics use the value resolved for the current file, and a scoped value replaces the base value.

```json
{
  "namespaceImportPackages": ["effect"],
  "overrides": [{
    "include": ["src/legacy/**/*.ts"],
    "options": {
      "namespaceImportPackages": [],
      "barrelImportPackages": ["effect"]
    }
  }]
}
```
