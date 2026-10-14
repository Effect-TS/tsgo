// @filename: package.json
{ "name": "@effect/test-app", "version": "1.0.0" }

// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "barrelImportPackages": ["@acme/ui", "@acme/icons"],
        "namespaceImportPackages": ["@acme/kit"]
      }
    ]
  }
}

// @filename: node_modules/@acme/ui/package.json
{ "name": "@acme/ui", "version": "1.0.0", "types": "./dist/index.d.ts", "exports": { ".": { "types": "./dist/index.d.ts" }, "./forms": { "types": "./dist/forms/index.d.ts" }, "./*": { "types": "./dist/*.d.ts" } } }

// @filename: node_modules/@acme/ui/dist/index.d.ts
export * as Button from "./Button.js"
export * as Http from "./HttpClient.js"

// @filename: node_modules/@acme/ui/dist/Button.d.ts
export declare const make: (label: string) => string

// @filename: node_modules/@acme/ui/dist/HttpClient.d.ts
export declare const get: (url: string) => string

// @filename: node_modules/@acme/ui/dist/Hidden.d.ts
export declare const secret: string

// @filename: node_modules/@acme/ui/dist/forms/index.d.ts
export * as TextInput from "./TextInput.js"

// @filename: node_modules/@acme/ui/dist/forms/TextInput.d.ts
export declare const make: () => string

// @filename: node_modules/@acme/icons/package.json
{ "name": "@acme/icons", "version": "1.0.0", "types": "./dist/index.d.ts", "exports": { ".": { "types": "./dist/index.d.ts" }, "./*": { "types": "./dist/*.d.ts" } } }

// @filename: node_modules/@acme/icons/dist/index.d.ts
export * as Arrow from "./Arrow.js"

// @filename: node_modules/@acme/icons/dist/Arrow.d.ts
export declare const left: string

// @filename: node_modules/@acme/kit/package.json
{ "name": "@acme/kit", "version": "1.0.0", "types": "./dist/index.d.ts", "exports": { ".": { "types": "./dist/index.d.ts" }, "./*": { "types": "./dist/*.d.ts" } } }

// @filename: node_modules/@acme/kit/dist/index.d.ts
export * as Dialog from "./Dialog.js"

// @filename: node_modules/@acme/kit/dist/Dialog.d.ts
export declare const open: () => void

// @filename: test.ts
// @effect-diagnostics *:off
// @effect-diagnostics preferBarrelImport:warning
// @effect-diagnostics preferNamespaceImport:warning

// not reported: barrels
import { Button } from "@acme/ui"
import { TextInput } from "@acme/ui/forms"

// reported by preferBarrelImport: forwarded by the root barrel
import * as B from "@acme/ui/Button"
// reported by preferBarrelImport: the barrel forwards HttpClient as Http
import { get } from "@acme/ui/HttpClient"
// reported by preferBarrelImport: forwarded by the nested barrel
import * as Input from "@acme/ui/forms/TextInput"
// not reported: no barrel forwards Hidden
import { secret } from "@acme/ui/Hidden"
// reported by preferBarrelImport: the @acme/icons barrel is read from disk, nothing imports it
import * as Arrow from "@acme/icons/Arrow"

// reported by preferNamespaceImport: namespace re-exported by the barrel
import { Dialog } from "@acme/kit"
// reported by preferNamespaceImport: declared in the imported module
import { open } from "@acme/kit/Dialog"

export const all = [Button.make("a"), TextInput.make(), B.make("b"), get("/"), Input.make(), secret, Arrow.left, Dialog.open, open]
