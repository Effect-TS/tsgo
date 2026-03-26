package effecttest_test

import (
	"testing"

	"github.com/microsoft/typescript-go/shim/fourslash"

	_ "github.com/effect-ts/tsgo/etslshooks"
	_ "github.com/effect-ts/tsgo/etstesthooks"
)

func TestEffectHoverYieldStar(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect } from "effect"
const program = Effect.gen(function*() {
  const result = /*1*/yield* Effect.succeed("hello")
})`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	f.VerifyQuickInfoAt(t, "1",
		"(yield*) Effect<string, never, never>",
		"```ts\n/* Effect Type Parameters */\ntype Success = string\ntype Failure = never\ntype Requirements = never\n```\n",
	)
}

func TestEffectHoverTypeArgs(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect } from "effect"
declare const /*1*/myEffect: Effect.Effect<string, Error, never>
declare const /*2*/notAnEffect: string`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	// Hover over an Effect-typed variable should include type parameters
	f.VerifyQuickInfoAt(t, "1",
		"const myEffect: Effect.Effect<string, Error, never>",
		"```ts\n/* Effect Type Parameters */\ntype Success = string\ntype Failure = Error\ntype Requirements = never\n```\n",
	)

	// Hover over a non-Effect-typed variable should have no documentation
	f.VerifyQuickInfoAt(t, "2",
		"const notAnEffect: string",
		"",
	)
}

func TestEffectHoverLayerQuickInfo(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect, Layer, ServiceMap } from "effect"

class Database extends ServiceMap.Service<Database>()("Database", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}

class Cache extends ServiceMap.Service<Cache>()("Cache", {
  make: Effect.as(Database.asEffect(), {})
}) {
  static Default = Layer.effect(this, this.make)
}

const /*1*/app = Cache.Default.pipe(Layer.provide(Database.Default))
declare const /*2*/myEffect: Effect.Effect<string, Error, never>`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	// Hover over a Layer-typed variable should show providers/requirers summary,
	// Mermaid diagram links, and Layer type parameters
	f.VerifyQuickInfoAt(t, "1",
		"const app: Layer.Layer<Cache, never, never>",
		"```\n/**\n * - Cache provided at ln 15 col 12 by `Cache.Default`\n */\n```\n"+
			"[Show full graph](https://mermaid.live/edit#pako:eNqckk9TszAQxr_KTk7ve2hLEuuhR-3Rg-N4A4duaWoZY8AQrB3H7-7wpxgqE4K3JfvwZPN79pMk2U6QFdnL7JgcUBt4vIkUQFFunzXmBwggjMhmjQa3WIj5WuyxlCYqg4AnW72oC9F8Fq8oZXMQSwV0CUkm4WoZN-2F1d9E5Km6xr4oPmrMw4hA1-t1c529pztRQHjfVp0KrHYchBE5j2s5CbXrTM1JCtuxMDp7ESuVKdFoOvFZWo0G-1TKldGoihy1UGZAcuHU-rSSQYPuhbQGfYcnoeftZP8usf-fwJ1TP-7UyZ26udM-91tMDk7o1B86HYdOx6FTN3RWQ6_H_sNqU-aHmDkRs1iLtzLVFeKHtrIQ_7S9V9ty_EW5d7EzWzYxW-afLRvPlo1ny9zZ8jpbzPMJiV77BcqdgXI3Vz6RK_fnyse58nGufNCAwmw--4Cgrq0nVMcNqN7GVDI-LLP_Jl_fAQAA__-x8ibJ) - [Show outline](https://mermaid.live/edit#pako:eNqqVkrOT0lVslJKy8kvT85ILCpRCHGJyVNQMIiOUXJJLElMSixO1XNJTUsszSmJUYoFSRlGxyg5JyZnYIgr6OrGlBoYGKcqGCjVAgIAAP__QH8cUg)\n\n",
	)

	// Hover over a plain Effect-typed variable should show Effect type parameters (not Layer)
	f.VerifyQuickInfoAt(t, "2",
		"const myEffect: Effect.Effect<string, Error, never>",
		"```ts\n/* Effect Type Parameters */\ntype Success = string\ntype Failure = Error\ntype Requirements = never\n```\n",
	)
}

func TestEffectHoverLayerNoExternal(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "noExternal": true
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect, Layer, ServiceMap } from "effect"

class Database extends ServiceMap.Service<Database>()("Database", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}

class Cache extends ServiceMap.Service<Cache>()("Cache", {
  make: Effect.as(Database.asEffect(), {})
}) {
  static Default = Layer.effect(this, this.make)
}

const /*1*/app = Cache.Default.pipe(Layer.provide(Database.Default))`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	// With noExternal=true, hover should show summary and type params but no Mermaid links
	f.VerifyQuickInfoAt(t, "1",
		"const app: Layer.Layer<Cache, never, never>",
		"```\n/**\n * - Cache provided at ln 15 col 12 by `Cache.Default`\n */\n```\n",
	)
}

func TestEffectHoverLayerNameOnly(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect, Layer, ServiceMap } from "effect"

class Database extends ServiceMap.Service<Database>()("Database", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}

class Cache extends ServiceMap.Service<Cache>()("Cache", {
  make: Effect.as(Database.asEffect(), {})
}) {
  static Default = Layer.effect(this, this.make)
}

const /*1*/app = Cache.Default.pipe(Layer.provide(Database.Default))
const app2 = /*2*/app`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	// Marker 1: on the variable name — should show full Layer hover enrichment
	f.VerifyQuickInfoAt(t, "1",
		"const app: Layer.Layer<Cache, never, never>",
		"```\n/**\n * - Cache provided at ln 15 col 12 by `Cache.Default`\n */\n```\n"+
			"[Show full graph](https://mermaid.live/edit#pako:eNqckk9TszAQxr_KTk7ve2hLEuuhR-3Rg-N4A4duaWoZY8AQrB3H7-7wpxgqE4K3JfvwZPN79pMk2U6QFdnL7JgcUBt4vIkUQFFunzXmBwggjMhmjQa3WIj5WuyxlCYqg4AnW72oC9F8Fq8oZXMQSwV0CUkm4WoZN-2F1d9E5Km6xr4oPmrMw4hA1-t1c529pztRQHjfVp0KrHYchBE5j2s5CbXrTM1JCtuxMDp7ESuVKdFoOvFZWo0G-1TKldGoihy1UGZAcuHU-rSSQYPuhbQGfYcnoeftZP8usf-fwJ1TP-7UyZ26udM-91tMDk7o1B86HYdOx6FTN3RWQ6_H_sNqU-aHmDkRs1iLtzLVFeKHtrIQ_7S9V9ty_EW5d7EzWzYxW-afLRvPlo1ny9zZ8jpbzPMJiV77BcqdgXI3Vz6RK_fnyse58nGufNCAwmw--4Cgrq0nVMcNqN7GVDI-LLP_Jl_fAQAA__-x8ibJ) - [Show outline](https://mermaid.live/edit#pako:eNqqVkrOT0lVslJKy8kvT85ILCpRCHGJyVNQMIiOUXJJLElMSixO1XNJTUsszSmJUYoFSRlGxyg5JyZnYIgr6OrGlBoYGKcqGCjVAgIAAP__QH8cUg)\n\n",
	)

	// Marker 2: on a reference to `app` in the initializer of another variable declaration.
	// The node's parent is the VariableDeclaration of `app2`, but the node itself is NOT
	// the declaration name. Layer hover enrichment should NOT activate — the hover shows
	// only the standard quickInfo type signature without providers/requirers or Mermaid links.
	f.VerifyQuickInfoAt(t, "2",
		"const app: Layer.Layer<Cache, never, never>",
		"",
	)
}

func TestEffectHoverLayerMermaidProvider(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "mermaidProvider": "mermaid.com"
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect, Layer, ServiceMap } from "effect"

class Database extends ServiceMap.Service<Database>()("Database", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}

class Cache extends ServiceMap.Service<Cache>()("Cache", {
  make: Effect.as(Database.asEffect(), {})
}) {
  static Default = Layer.effect(this, this.make)
}

const /*1*/app = Cache.Default.pipe(Layer.provide(Database.Default))`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	// With mermaidProvider="mermaid.com", links should use mermaidchart.com URL
	f.VerifyQuickInfoAt(t, "1",
		"const app: Layer.Layer<Cache, never, never>",
		"```\n/**\n * - Cache provided at ln 15 col 12 by `Cache.Default`\n */\n```\n"+
			"[Show full graph](https://www.mermaidchart.com/play#pako:eNqckk9TszAQxr_KTk7ve2hLEuuhR-3Rg-N4A4duaWoZY8AQrB3H7-7wpxgqE4K3JfvwZPN79pMk2U6QFdnL7JgcUBt4vIkUQFFunzXmBwggjMhmjQa3WIj5WuyxlCYqg4AnW72oC9F8Fq8oZXMQSwV0CUkm4WoZN-2F1d9E5Km6xr4oPmrMw4hA1-t1c529pztRQHjfVp0KrHYchBE5j2s5CbXrTM1JCtuxMDp7ESuVKdFoOvFZWo0G-1TKldGoihy1UGZAcuHU-rSSQYPuhbQGfYcnoeftZP8usf-fwJ1TP-7UyZ26udM-91tMDk7o1B86HYdOx6FTN3RWQ6_H_sNqU-aHmDkRs1iLtzLVFeKHtrIQ_7S9V9ty_EW5d7EzWzYxW-afLRvPlo1ny9zZ8jpbzPMJiV77BcqdgXI3Vz6RK_fnyse58nGufNCAwmw--4Cgrq0nVMcNqN7GVDI-LLP_Jl_fAQAA__-x8ibJ) - [Show outline](https://www.mermaidchart.com/play#pako:eNqqVkrOT0lVslJKy8kvT85ILCpRCHGJyVNQMIiOUXJJLElMSixO1XNJTUsszSmJUYoFSRlGxyg5JyZnYIgr6OrGlBoYGKcqGCjVAgIAAP__QH8cUg)\n\n",
	)
}
