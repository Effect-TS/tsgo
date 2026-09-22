package effecttest_test

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/fourslash"

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
import { Config, Effect } from "effect"
const program = Effect.gen(function*() {
  const token = /*yield*/yield/*asterisk*/*/*space*/ Config.redacted("WEBHOOK_TOKEN")
})`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	for _, marker := range []string{"yield", "asterisk", "space"} {
		f.VerifyQuickInfoAt(t, marker,
			"(yield*) Config<Redacted<string>>",
			"```ts\n/* Effect Type Parameters */\ntype Success = Redacted<string>\ntype Failure = ConfigError\ntype Requirements = never\n```\n",
		)
	}
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
import { Effect, Layer, Context } from "effect"

class Database extends Context.Service<Database>()("Database", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}

class Cache extends Context.Service<Cache>()("Cache", {
  make: Effect.as(Database, {})
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
			"[Show full graph](https://mermaid.live/edit#pako:eNqckk9T8jAQxr_KTk7vewCaRDxwVI4eHMdbw5SlBGGMaU1bkXH87k7_UFLspKm3bffJk83v2S8SJ1tJFmSnkmO8R5PD853QAFmxeTGY7iGAUJD1EnPcYCanS7nDQuWiCAIeb8ysKmT9mb2hUvWPSGmgc4gTBTfzqG7PrP5akJXQAPZF0dFgGgoCba_TTU3ycdjKDMLHpmpVYLWjIBTkPK7lJPW2Nc1PStqOWW6SV7nQiZZCd8RnaTka7A5KLXKDOkvRSJ33SK6cGp9G0mvQvpBWoB_wJM20mezfNfb_I7hz6sedOrlTN3fa5X6P8d4JnfpDp8PQ6TB06obOKujV2H9Ybcr8EDMnYhYZ-V4cTIn4qalWF8SXtvdqX478pty52JktG5kt88-WDWfLhrNl7mx5lS2m6YhEb_0C5c5AuZsrH8mV-3Plw1z5MFfea0BhMp18QlDV1hPK3zWozsaUMt4vs0-T758BALHyJsk) - [Show outline](https://mermaid.live/edit#pako:eNqqVkrOT0lVslJKy8kvT85ILCpRCHGJyVNQMIiOUXJJLElMSixO1XNJTUsszSmJUYoFSRlGxyg5JyZnYIgr6OrGlBoYGKcqGCjVAgYAQH8cUg)\n\n",
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
import { Effect, Layer, Context } from "effect"

class Database extends Context.Service<Database>()("Database", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}

class Cache extends Context.Service<Cache>()("Cache", {
  make: Effect.as(Database, {})
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
import { Effect, Layer, Context } from "effect"

class Database extends Context.Service<Database>()("Database", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}

class Cache extends Context.Service<Cache>()("Cache", {
  make: Effect.as(Database, {})
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
			"[Show full graph](https://mermaid.live/edit#pako:eNqckk9T8jAQxr_KTk7vewCaRDxwVI4eHMdbw5SlBGGMaU1bkXH87k7_UFLspKm3bffJk83v2S8SJ1tJFmSnkmO8R5PD853QAFmxeTGY7iGAUJD1EnPcYCanS7nDQuWiCAIeb8ysKmT9mb2hUvWPSGmgc4gTBTfzqG7PrP5akJXQAPZF0dFgGgoCba_TTU3ycdjKDMLHpmpVYLWjIBTkPK7lJPW2Nc1PStqOWW6SV7nQiZZCd8RnaTka7A5KLXKDOkvRSJ33SK6cGp9G0mvQvpBWoB_wJM20mezfNfb_I7hz6sedOrlTN3fa5X6P8d4JnfpDp8PQ6TB06obOKujV2H9Ybcr8EDMnYhYZ-V4cTIn4qalWF8SXtvdqX478pty52JktG5kt88-WDWfLhrNl7mx5lS2m6YhEb_0C5c5AuZsrH8mV-3Plw1z5MFfea0BhMp18QlDV1hPK3zWozsaUMt4vs0-T758BALHyJsk) - [Show outline](https://mermaid.live/edit#pako:eNqqVkrOT0lVslJKy8kvT85ILCpRCHGJyVNQMIiOUXJJLElMSixO1XNJTUsszSmJUYoFSRlGxyg5JyZnYIgr6OrGlBoYGKcqGCjVAgYAQH8cUg)\n\n",
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

func TestEffectHoverDisabled(t *testing.T) {
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
        "quickinfo": false
      }
    ]
  }
}
// @Filename: /test.ts
import { Effect } from "effect"
declare const /*1*/myEffect: Effect.Effect<string, Error, never>`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	f.VerifyQuickInfoAt(t, "1",
		"const myEffect: Effect.Effect<string, Error, never>",
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
import { Effect, Layer, Context } from "effect"

class Database extends Context.Service<Database>()("Database", {
  make: Effect.succeed({})
}) {
  static Default = Layer.effect(this, this.make)
}

class Cache extends Context.Service<Cache>()("Cache", {
  make: Effect.as(Database, {})
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
			"[Show full graph](https://www.mermaidchart.com/play#pako:eNqckk9T8jAQxr_KTk7vewCaRDxwVI4eHMdbw5SlBGGMaU1bkXH87k7_UFLspKm3bffJk83v2S8SJ1tJFmSnkmO8R5PD853QAFmxeTGY7iGAUJD1EnPcYCanS7nDQuWiCAIeb8ysKmT9mb2hUvWPSGmgc4gTBTfzqG7PrP5akJXQAPZF0dFgGgoCba_TTU3ycdjKDMLHpmpVYLWjIBTkPK7lJPW2Nc1PStqOWW6SV7nQiZZCd8RnaTka7A5KLXKDOkvRSJ33SK6cGp9G0mvQvpBWoB_wJM20mezfNfb_I7hz6sedOrlTN3fa5X6P8d4JnfpDp8PQ6TB06obOKujV2H9Ybcr8EDMnYhYZ-V4cTIn4qalWF8SXtvdqX478pty52JktG5kt88-WDWfLhrNl7mx5lS2m6YhEb_0C5c5AuZsrH8mV-3Plw1z5MFfea0BhMp18QlDV1hPK3zWozsaUMt4vs0-T758BALHyJsk) - [Show outline](https://www.mermaidchart.com/play#pako:eNqqVkrOT0lVslJKy8kvT85ILCpRCHGJyVNQMIiOUXJJLElMSixO1XNJTUsszSmJUYoFSRlGxyg5JyZnYIgr6OrGlBoYGKcqGCjVAgYAQH8cUg)\n\n",
	)
}
