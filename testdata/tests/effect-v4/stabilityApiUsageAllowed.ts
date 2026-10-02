// @filename: tsconfig.json
{
  "compilerOptions": {
    "plugins": [{
      "name": "@effect/language-service",
      "allowedUnstableApis": ["allowed/http/HttpClient#get", "allowed/rpc", "effect/http/HttpClient#get"],
      "allowedExperimentalApis": ["allowed/http/HttpClient#preview", "allowed/http/HttpClient#experimentalFetch", "allowed/experimental"],
      "overrides": [{
        "include": ["module.ts"],
        "options": {
          "allowedUnstableApis": ["allowed/http/HttpClient"],
          "allowedExperimentalApis": ["allowed/http/HttpClient"]
        }
      }, {
        "include": ["exportAlias.ts"],
        "options": {
          "allowedUnstableApis": ["allowed/http/HttpClient#send"],
          "allowedExperimentalApis": []
        }
      }, {
        "include": ["experimentalOnly.ts"],
        "options": {
          "allowedUnstableApis": [],
          "allowedExperimentalApis": ["allowed/http/HttpClient", "effect/http"]
        }
      }, {
        "include": ["realWorldModules.ts"],
        "options": { "allowedUnstableApis": ["effect/http/HttpClient"] }
      }, {
        "include": ["realWorldSubtree.ts"],
        "options": { "allowedUnstableApis": ["effect/http"] }
      }]
    }]
  }
}

// @filename: /node_modules/allowed/package.json
{ "name": "allowed", "version": "1.0.0" }

// @filename: /node_modules/allowed/dist/http/HttpClient.d.ts
/** @stability unstable */
export declare function get(value: string): string
export declare function get(value: number): number
/** @stability unstable */
export declare const post: () => void
/** @stability experimental */
export declare const experimental: () => void
/** @stability experimental */
export declare const otherExperimental: () => void
/** @stability experimental */
export declare function experimentalGet(value: string): string
export declare function experimentalGet(value: number): number
export { post as send, experimental as preview, experimentalGet as experimentalFetch }

// @filename: /node_modules/allowed/dist/rpc/Client.d.ts
/** @stability unstable */
export declare const make: () => void

// @filename: /node_modules/allowed/dist/rpc-extra/Client.d.ts
/** @stability unstable */
export declare const make: () => void

// @filename: /node_modules/allowed/dist/experimental/Client.d.ts
/** @stability experimental */
export declare const make: () => void

// @filename: /node_modules/allowed/dist/experimental-extra/Client.d.ts
/** @stability experimental */
export declare const make: () => void

// @filename: barrel.ts
export { get as fetch, post } from "allowed/dist/http/HttpClient"

// @filename: consumer.ts
// @effect-v4
// @effect-diagnostics unstableApiUsage:warning experimentalApiUsage:warning
import { get as renamed, post, send, experimental, preview, otherExperimental, experimentalGet as experimentalAlias } from "allowed/dist/http/HttpClient"
import * as Client from "allowed/dist/http/HttpClient"
import { fetch } from "./barrel"
import { make } from "allowed/dist/rpc/Client"
import { make as other } from "allowed/dist/rpc-extra/Client"
import { make as experimentalMake } from "allowed/dist/experimental/Client"
import { make as otherExperimentalMake } from "allowed/dist/experimental-extra/Client"

renamed("hello")
renamed(1)
Client.get("hello")
export const result1 = fetch("hello")
const alias = renamed
alias("hello")
post()
post()
send()
experimental()
preview()
experimental()
Client.experimental()
const previewAlias = preview
previewAlias()
experimentalAlias("hello")
experimentalAlias(1)
otherExperimental()
otherExperimental()
experimentalMake()
otherExperimentalMake()
make()
other()

// @filename: module.ts
// @effect-v4
// @effect-diagnostics unstableApiUsage:warning experimentalApiUsage:warning
import { get, post, experimental, otherExperimental } from "allowed/dist/http/HttpClient"
get("hello")
post()
experimental()
otherExperimental()

// @filename: exportAlias.ts
// @effect-v4
// @effect-diagnostics unstableApiUsage:warning experimentalApiUsage:warning
import { post, send, get, experimental } from "allowed/dist/http/HttpClient"
post()
send()
get("still unstable")
get(1)
experimental()
experimental()

// @filename: experimentalOnly.ts
// @effect-v4
// @effect-diagnostics unstableApiUsage:warning experimentalApiUsage:warning
import { get, post, experimental, otherExperimental } from "allowed/dist/http/HttpClient"
import { HttpClient } from "effect/http"
get("still unstable")
post()
experimental()
otherExperimental()
export const result1 = HttpClient.get("https://example.com")

// @filename: realWorldMembers.ts
// @effect-v4
// @effect-diagnostics unstableApiUsage:warning experimentalApiUsage:warning
import { Effect } from "effect"
import { HttpClient } from "effect/http"
import * as DirectClient from "effect/http/HttpClient"
import { get as fetch } from "effect/http/HttpClient"

// All get references resolve to effect/http/HttpClient#get.
export const result1 = HttpClient.get("https://example.com")
export const result2 = DirectClient.get("https://example.com")
export const result3 = fetch("https://example.com")
const fetchAlias = fetch
export const result4 = fetchAlias("https://example.com")
// Unlisted exports still warn, including repeated references.
export const result5 = HttpClient.post("https://example.com")
export const result6 = DirectClient.post("https://example.com")
export const result7 = Effect.succeed(1)

// @filename: realWorldModules.ts
// @effect-v4
// @effect-diagnostics unstableApiUsage:warning
import { HttpClient, HttpClientRequest } from "effect/http"
export const result1 = HttpClient.get("https://example.com")
export const result2 = HttpClient.post("https://example.com")
// Allowing HttpClient does not allow a sibling module.
export const result3 = HttpClientRequest.get("https://example.com")

// @filename: realWorldSubtree.ts
// @effect-v4
// @effect-diagnostics unstableApiUsage:warning
import { HttpClient, HttpClientRequest } from "effect/http"
import { HttpApi } from "effect/http-api"
// The http subtree includes both declaration modules.
export const result1 = HttpClient.get("https://example.com")
export const result2 = HttpClient.post("https://example.com")
export const result3 = HttpClientRequest.get("https://example.com")
// A path-segment match must not allow the separate http-api subtree.
export const api = HttpApi.make("example")
