// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}

// @filename: /node_modules/my-effect-lib/package.json
{ "name": "my-effect-lib", "version": "1.0.0", "peerDependencies": { "effect": "*" } }

// @filename: /node_modules/my-effect-lib/index.d.ts
export declare const foo: number

// @filename: /node_modules/wrapper/package.json
{ "name": "wrapper", "version": "1.0.0" }

// @filename: /node_modules/wrapper/index.d.ts
export { bar } from "my-effect-lib"

// @filename: /node_modules/wrapper/node_modules/my-effect-lib/package.json
{ "name": "my-effect-lib", "version": "2.0.0", "peerDependencies": { "effect": "*" } }

// @filename: /node_modules/wrapper/node_modules/my-effect-lib/index.d.ts
export declare const bar: number

// @filename: test.ts
import { Effect } from "effect"
import { foo } from "my-effect-lib"
import { bar } from "wrapper"

const program = Effect.succeed(foo + bar)
