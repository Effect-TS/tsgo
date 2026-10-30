// Rust: Go's blank imports of `etslshooks` and `etstesthooks`: each test registers them (`ets_effecttest::init_hooks`).

use std::sync::LazyLock;

use ets_fourslash::FourslashTest;
use ets_go::testing::T;
use libtest_mimic::Trial;
use regex::Regex;

use crate::run_case;

/// Rust-only: `porting/DIVERGENCES.md`, `layergraph.EncodeMermaidURL`: the `pako:` fragment of a Mermaid link holds
/// zlib-compressed bytes that differ from Go's (an equally valid deflate encoding of the same payload). Hover
/// expectations with such links are compared after replacing each fragment with its decompressed payload, as the
/// layergraph tests compare the decoded roundtrip.
fn decode_mermaid_links(s: &str) -> String {
    static PAKO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"pako:([A-Za-z0-9_-]+)").unwrap());
    PAKO.replace_all(s, |caps: &regex::Captures<'_>| {
        let compressed = base64_raw_url_decode(&caps[1]);
        let mut decompressed = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
            &mut decompressed,
        )
        .expect("a zlib stream");
        format!("pako-payload:{}", String::from_utf8_lossy(&decompressed))
    })
    .into_owned()
}

/// Rust-only: Go's `base64.RawURLEncoding.DecodeString` for a well-formed fragment.
fn base64_raw_url_decode(s: &str) -> Vec<u8> {
    let value = |b: u8| -> u32 {
        match b {
            b'A'..=b'Z' => u32::from(b - b'A'),
            b'a'..=b'z' => u32::from(b - b'a') + 26,
            b'0'..=b'9' => u32::from(b - b'0') + 52,
            b'-' => 62,
            _ => 63,
        }
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.as_bytes().chunks(4) {
        let mut n = 0u32;
        for (i, &b) in chunk.iter().enumerate() {
            n |= value(b) << (18 - 6 * i);
        }
        out.extend_from_slice(&n.to_be_bytes()[1..chunk.len()]);
    }
    out
}

/// Rust-only: Go's `f.VerifyQuickInfoAt(t, marker, text, documentation)` for an expectation with Mermaid links,
/// compared with the links decoded (see `decode_mermaid_links`).
fn verify_quick_info_at_decoded(
    f: &mut FourslashTest,
    t: &T,
    marker: &str,
    expected_text: &str,
    expected_documentation: &str,
) {
    f.go_to_marker(t, marker);
    let Some(hover) = f.get_quick_info_at_current_position(t) else {
        t.fatal(format!("Expected hover result at marker '{marker}' but got nil"));
    };
    let Some(markup_content) = &hover.contents.markup_content else {
        t.fatal(format!("{}Expected markup content", f.get_current_position_prefix()));
    };
    ets_fourslash::verify_hover_markdown(
        t,
        &decode_mermaid_links(&markup_content.value),
        expected_text,
        &decode_mermaid_links(expected_documentation),
        &f.get_current_position_prefix(),
    );
}

// go test: internal/effecttest/hover_test.go:12 pluginTestContent 970991a901975fc8
pub(crate) fn plugin_test_content(plugin_options: &str, source: &str) -> String {
    r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service""#
        .to_string()
        + plugin_options
        + r"
      }
    ]
  }
}
// @Filename: /test.ts
" + source
}

// go test: internal/effecttest/hover_test.go:31 hoverLayerSource 4c5bbdbaf675a09a
const HOVER_LAYER_SOURCE: &str = r#"import { Effect, Layer, Context } from "effect"

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

const /*1*/app = Cache.Default.pipe(Layer.provide(Database.Default))"#;

// go test: internal/effecttest/hover_test.go:47 hoverLayerSummary 56b586bd7b9f14b9
const HOVER_LAYER_SUMMARY: &str =
    "```\n/**\n * - Cache provided at ln 15 col 12 by `Cache.Default`\n */\n```\n";

// go test: internal/effecttest/hover_test.go:49 hoverLayerMermaidPayloads 59bb42bd945a767f
const HOVER_LAYER_MERMAID_PAYLOADS: &str = "#pako:eNqckk9T8jAQxr_KTk7vewCaRDxwVI4eHMdbw5SlBGGMaU1bkXH87k7_UFLspKm3bffJk83v2S8SJ1tJFmSnkmO8R5PD853QAFmxeTGY7iGAUJD1EnPcYCanS7nDQuWiCAIeb8ysKmT9mb2hUvWPSGmgc4gTBTfzqG7PrP5akJXQAPZF0dFgGgoCba_TTU3ycdjKDMLHpmpVYLWjIBTkPK7lJPW2Nc1PStqOWW6SV7nQiZZCd8RnaTka7A5KLXKDOkvRSJ33SK6cGp9G0mvQvpBWoB_wJM20mezfNfb_I7hz6sedOrlTN3fa5X6P8d4JnfpDp8PQ6TB06obOKujV2H9Ybcr8EDMnYhYZ-V4cTIn4qalWF8SXtvdqX478pty52JktG5kt88-WDWfLhrNl7mx5lS2m6YhEb_0C5c5AuZsrH8mV-3Plw1z5MFfea0BhMp18QlDV1hPK3zWozsaUMt4vs0-T758BALHyJsk";

// go test: internal/effecttest/hover_test.go:51 hoverLayerOutlinePayload c8de3fd054ce4d02
const HOVER_LAYER_OUTLINE_PAYLOAD: &str = "#pako:eNqqVkrOT0lVslJKy8kvT85ILCpRCHGJyVNQMIiOUXJJLElMSixO1XNJTUsszSmJUYoFSRlGxyg5JyZnYIgr6OrGlBoYGKcqGCjVAgYAQH8cUg";

// go test: internal/effecttest/hover_test.go:53 hoverLayerLinks a9c2d64a33f76038
fn hover_layer_links(base_url: &str) -> String {
    "[Show full graph](".to_string()
        + base_url
        + HOVER_LAYER_MERMAID_PAYLOADS
        + ") - [Show outline]("
        + base_url
        + HOVER_LAYER_OUTLINE_PAYLOAD
        + ")\n\n"
}

// go test: internal/effecttest/hover_test.go:57 hoverEffectTypeParams 0cc32dfb30c03ed5
const HOVER_EFFECT_TYPE_PARAMS: &str = "```ts\n/* Effect Type Parameters */\ntype Success = string\ntype Failure = Error\ntype Requirements = never\n```\n";

// go test: internal/effecttest/hover_test.go:59 TestEffectHoverYieldStar 59fb771c5e4785eb
fn test_effect_hover_yield_star(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let mut f = ets_fourslash::new_fourslash(
        t,
        None,
        &plugin_test_content(
            "",
            r#"import { Config, Effect } from "effect"
const program = Effect.gen(function*() {
  const token = /*yield*/yield/*asterisk*/*/*space*/ Config.Redacted("WEBHOOK_TOKEN")
})"#,
        ),
    );

    for marker in ["yield", "asterisk", "space"] {
        f.verify_quick_info_at(
            t,
            marker,
            "(yield*) Config<Redacted<string>>",
            "```ts\n/* Effect Type Parameters */\ntype Success = Redacted<string>\ntype Failure = ConfigError\ntype Requirements = never\n```\n",
        );
    }
    f.done(t);
}

// go test: internal/effecttest/hover_test.go:76 TestEffectHoverTypeArgs 60112e05ae04920a
fn test_effect_hover_type_args(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let mut f = ets_fourslash::new_fourslash(
        t,
        None,
        &plugin_test_content(
            "",
            r#"import { Effect } from "effect"
declare const /*1*/myEffect: Effect.Effect<string, Error, never>
declare const /*2*/notAnEffect: string"#,
        ),
    );

    f.verify_quick_info_at(
        t,
        "1",
        "const myEffect: Effect.Effect<string, Error, never>",
        HOVER_EFFECT_TYPE_PARAMS,
    );

    f.verify_quick_info_at(t, "2", "const notAnEffect: string", "");
    f.done(t);
}

// go test: internal/effecttest/hover_test.go:89 TestEffectHoverDisabled 3c728a6c8fcd4d88
fn test_effect_hover_disabled(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let mut f = ets_fourslash::new_fourslash(
        t,
        None,
        &plugin_test_content(
            r#",
        "quickinfo": false"#,
            r#"import { Effect } from "effect"
declare const /*1*/myEffect: Effect.Effect<string, Error, never>"#,
        ),
    );

    f.verify_quick_info_at(t, "1", "const myEffect: Effect.Effect<string, Error, never>", "");
    f.done(t);
}

// go test: internal/effecttest/hover_test.go:100 TestEffectHoverLayerQuickInfo 874d3f7b9646eb65
fn test_effect_hover_layer_quick_info(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let mut f = ets_fourslash::new_fourslash(
        t,
        None,
        &plugin_test_content("", &(HOVER_LAYER_SOURCE.to_string() + "\nconst app2 = /*2*/app")),
    );

    // Rust-only: Mermaid links compared decoded (porting/DIVERGENCES.md).
    verify_quick_info_at_decoded(
        &mut f,
        t,
        "1",
        "const app: Layer.Layer<Cache, never, never>",
        &(HOVER_LAYER_SUMMARY.to_string() + &hover_layer_links("https://mermaid.live/edit")),
    );

    f.verify_quick_info_at(t, "2", "const app: Layer.Layer<Cache, never, never>", "");
    f.done(t);
}

// go test: internal/effecttest/hover_test.go:111 TestEffectHoverLayerNoExternal 3feb66928aa16674
fn test_effect_hover_layer_no_external(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let mut f = ets_fourslash::new_fourslash(
        t,
        None,
        &plugin_test_content(
            r#",
        "noExternal": true"#,
            HOVER_LAYER_SOURCE,
        ),
    );

    f.verify_quick_info_at(
        t,
        "1",
        "const app: Layer.Layer<Cache, never, never>",
        HOVER_LAYER_SUMMARY,
    );
    f.done(t);
}

// go test: internal/effecttest/hover_test.go:121 TestEffectHoverLayerMermaidProvider 22d9f38e9c1c4832
fn test_effect_hover_layer_mermaid_provider(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();

    let mut f = ets_fourslash::new_fourslash(
        t,
        None,
        &plugin_test_content(
            r#",
        "mermaidProvider": "mermaid.com""#,
            HOVER_LAYER_SOURCE,
        ),
    );

    // Rust-only: Mermaid links compared decoded (porting/DIVERGENCES.md).
    verify_quick_info_at_decoded(
        &mut f,
        t,
        "1",
        "const app: Layer.Layer<Cache, never, never>",
        &(HOVER_LAYER_SUMMARY.to_string()
            + &hover_layer_links("https://www.mermaidchart.com/play")),
    );
    f.done(t);
}

/// Rust-only: the trials of this file, one per Go test, named like it.
pub(crate) fn tests() -> Vec<Trial> {
    vec![
        run_case("TestEffectHoverYieldStar".to_string(), test_effect_hover_yield_star),
        run_case("TestEffectHoverTypeArgs".to_string(), test_effect_hover_type_args),
        run_case("TestEffectHoverDisabled".to_string(), test_effect_hover_disabled),
        run_case("TestEffectHoverLayerQuickInfo".to_string(), test_effect_hover_layer_quick_info),
        run_case("TestEffectHoverLayerNoExternal".to_string(), test_effect_hover_layer_no_external),
        run_case(
            "TestEffectHoverLayerMermaidProvider".to_string(),
            test_effect_hover_layer_mermaid_provider,
        ),
    ]
}
