package effecttest_test

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/fourslash"

	_ "github.com/effect-ts/tsgo/etslshooks"
	_ "github.com/effect-ts/tsgo/etstesthooks"
)

func pluginTestContent(pluginOptions string, source string) string {
	return `// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service"` + pluginOptions + `
      }
    ]
  }
}
// @Filename: /test.ts
` + source
}

const hoverLayerSource = `import { Effect, Layer, Context } from "effect"

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

const hoverLayerSummary = "```\n/**\n * - Cache provided at ln 15 col 12 by `Cache.Default`\n */\n```\n"

const hoverLayerMermaidPayloads = "#pako:eNqckk9T8jAQxr_KTk7vewCaRDxwVI4eHMdbw5SlBGGMaU1bkXH87k7_UFLspKm3bffJk83v2S8SJ1tJFmSnkmO8R5PD853QAFmxeTGY7iGAUJD1EnPcYCanS7nDQuWiCAIeb8ysKmT9mb2hUvWPSGmgc4gTBTfzqG7PrP5akJXQAPZF0dFgGgoCba_TTU3ycdjKDMLHpmpVYLWjIBTkPK7lJPW2Nc1PStqOWW6SV7nQiZZCd8RnaTka7A5KLXKDOkvRSJ33SK6cGp9G0mvQvpBWoB_wJM20mezfNfb_I7hz6sedOrlTN3fa5X6P8d4JnfpDp8PQ6TB06obOKujV2H9Ybcr8EDMnYhYZ-V4cTIn4qalWF8SXtvdqX478pty52JktG5kt88-WDWfLhrNl7mx5lS2m6YhEb_0C5c5AuZsrH8mV-3Plw1z5MFfea0BhMp18QlDV1hPK3zWozsaUMt4vs0-T758BALHyJsk"

const hoverLayerOutlinePayload = "#pako:eNqqVkrOT0lVslJKy8kvT85ILCpRCHGJyVNQMIiOUXJJLElMSixO1XNJTUsszSmJUYoFSRlGxyg5JyZnYIgr6OrGlBoYGKcqGCjVAgYAQH8cUg"

func hoverLayerLinks(baseURL string) string {
	return "[Show full graph](" + baseURL + hoverLayerMermaidPayloads + ") - [Show outline](" + baseURL + hoverLayerOutlinePayload + ")\n\n"
}

const hoverEffectTypeParams = "```ts\n/* Effect Type Parameters */\ntype Success = string\ntype Failure = Error\ntype Requirements = never\n```\n"

func TestEffectHoverYieldStar(t *testing.T) {
	t.Parallel()

	f, done := fourslash.NewFourslash(t, nil, pluginTestContent("", `import { Config, Effect } from "effect"
const program = Effect.gen(function*() {
  const token = /*yield*/yield/*asterisk*/*/*space*/ Config.Redacted("WEBHOOK_TOKEN")
})`))
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

	f, done := fourslash.NewFourslash(t, nil, pluginTestContent("", `import { Effect } from "effect"
declare const /*1*/myEffect: Effect.Effect<string, Error, never>
declare const /*2*/notAnEffect: string`))
	defer done()

	f.VerifyQuickInfoAt(t, "1", "const myEffect: Effect.Effect<string, Error, never>", hoverEffectTypeParams)

	f.VerifyQuickInfoAt(t, "2", "const notAnEffect: string", "")
}

func TestEffectHoverDisabled(t *testing.T) {
	t.Parallel()

	f, done := fourslash.NewFourslash(t, nil, pluginTestContent(`,
        "quickinfo": false`, `import { Effect } from "effect"
declare const /*1*/myEffect: Effect.Effect<string, Error, never>`))
	defer done()

	f.VerifyQuickInfoAt(t, "1", "const myEffect: Effect.Effect<string, Error, never>", "")
}

func TestEffectHoverLayerQuickInfo(t *testing.T) {
	t.Parallel()

	f, done := fourslash.NewFourslash(t, nil, pluginTestContent("", hoverLayerSource+"\nconst app2 = /*2*/app"))
	defer done()

	f.VerifyQuickInfoAt(t, "1", "const app: Layer.Layer<Cache, never, never>", hoverLayerSummary+hoverLayerLinks("https://mermaid.live/edit"))

	f.VerifyQuickInfoAt(t, "2", "const app: Layer.Layer<Cache, never, never>", "")
}

func TestEffectHoverLayerNoExternal(t *testing.T) {
	t.Parallel()

	f, done := fourslash.NewFourslash(t, nil, pluginTestContent(`,
        "noExternal": true`, hoverLayerSource))
	defer done()

	f.VerifyQuickInfoAt(t, "1", "const app: Layer.Layer<Cache, never, never>", hoverLayerSummary)
}

func TestEffectHoverLayerMermaidProvider(t *testing.T) {
	t.Parallel()

	f, done := fourslash.NewFourslash(t, nil, pluginTestContent(`,
        "mermaidProvider": "mermaid.com"`, hoverLayerSource))
	defer done()

	f.VerifyQuickInfoAt(t, "1", "const app: Layer.Layer<Cache, never, never>", hoverLayerSummary+hoverLayerLinks("https://www.mermaidchart.com/play"))
}
