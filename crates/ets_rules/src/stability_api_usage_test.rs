// Ported from effect-tsgo's internal/rules/stability_api_usage_test.go (BenchmarkStabilityApiUsageContextualProperties
// is listed in porting/skipped.tsv).

use ets_etscore::ResolvedEffectPluginOptions;
use ets_go::testing::T;
use ets_rule::Rule;
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFileParseOptions};
use ets_shim::core::ScriptKind;

use crate::api_stability_leak_regression_test::*;
use crate::api_stability_leak_test::*;
use crate::*;

/// Rust-only: runs a ported test on a checker-sized stack.
fn run(f: impl FnOnce(&T) + Send + 'static) {
    ets_go::testing::on_checker_stack(move || ets_go::testing::run(f));
}

/// Rust-only: Go's `%v` of a diagnostic list in failure messages (the diagnostics' texts).
fn diagnostic_texts(diagnostics: &[P<Diagnostic>]) -> String {
    let texts: Vec<String> = diagnostics.iter().map(|diagnostic| diagnostic.localize()).collect();
    format!("{texts:?}")
}

/// Rust-only: Go's `diagnostic.Pos()` (an `int`).
fn diagnostic_pos(diagnostic: P<Diagnostic>) -> usize {
    diagnostic.pos().expect("a diagnostic with a location") as usize
}

/// Rust-only: Go's `source[diagnostic.Pos() : diagnostic.Pos()+diagnostic.Len()]`.
fn diagnostic_text(source: &str, diagnostic: P<Diagnostic>) -> &str {
    let pos = diagnostic_pos(diagnostic);
    let len = diagnostic.len().expect("a diagnostic with a location") as usize;
    &source[pos..pos + len]
}

// go test: internal/rules/stability_api_usage_test.go:17 TestStabilityOfDeclaration a79a007f5216824d
#[test]
fn test_stability_of_declaration() {
    run(|t| {
        t.parallel();
        let tests: &[(&str, &str, &str)] = &[
            ("unstable variable", "/** @stability unstable */\nexport const api = 1", "unstable"),
            (
                "experimental variable",
                "/** @stability experimental */\nexport const api = 1",
                "experimental",
            ),
            ("stable variable", "/** @stability stable */\nexport const api = 1", "stable"),
            ("other tag", "/** @deprecated */\nexport const api = 1", ""),
            ("plain variable", "export const api = 1", ""),
        ];
        for &(name, source, want) in tests {
            t.run(name, |t| {
                t.parallel();
                let sf = ets_shim::parser::parse_source_file(
                    SourceFileParseOptions {
                        file_name: "/test.ts".to_string(),
                        ..Default::default()
                    },
                    source,
                    ScriptKind::TS,
                );
                let statement = sf.statements().nodes()[0];
                let declaration = statement
                    .as_variable_statement()
                    .declaration_list()
                    .as_variable_declaration_list()
                    .declarations()
                    .nodes()[0];
                let got = ets_typeparser::stability_tag_of_declaration(declaration);
                if got != want {
                    t.error(ets_go::sprintf!("StabilityTagOfDeclaration = %q, want %q", got, want));
                }
            });
        }
    });
}

// go test: internal/rules/stability_api_usage_test.go:43 TestStabilityApiModuleName cf782c93864dd5ff
#[test]
fn test_stability_api_module_name() {
    run(|t| {
        t.parallel();
        for (file, want) in [
            ("/pkg/src/http/HttpClient.ts", "effect/http/HttpClient"),
            ("/pkg/dist/http/HttpClient.d.ts", "effect/http/HttpClient"),
            ("/pkg/dist/dts/http/HttpClient.d.mts", "effect/http/HttpClient"),
            ("/pkg/dist/cjs/http/HttpClient.d.cts", "effect/http/HttpClient"),
            ("/pkg/dist/http/index.d.ts", "effect/http"),
            ("/pkg/index.ts", "effect"),
            ("/pkg/types/client.d.ts", "effect/types/client"),
            ("/pkg-other/client.ts", ""),
        ] {
            t.run(file, |t| {
                t.parallel();
                let got = stability_api_module_name("effect", "/pkg", file);
                if got != want {
                    t.fatal(ets_go::sprintf!("got %q, want %q", got, want));
                }
            });
        }
    });
}

// go test: internal/rules/stability_api_usage_test.go:64 TestStabilityApiUsageContextualProperties 112ce46ce7ac871a
#[test]
fn test_stability_api_usage_contextual_properties() {
    run(|t| {
        t.parallel();
        let tests: &[(&str, &str, &str)] = &[
            ("argument", "x({ test: 42 })", "test"),
            ("shorthand", "const test = 42; x({ test })", "test"),
            ("quoted key", r#"x({ "test": 42 })"#, r#""test""#),
            ("computed literal key", r#"x({ ["test"]: 42 })"#, r#"["test"]"#),
            ("typed variable", "const value: Reportable = { test: 42 }", "test"),
            ("assignment", "let value: Reportable; value = { test: 42 }", "test"),
            ("satisfies", "const value = { test: 42 } satisfies Reportable", "test"),
            ("return", "function make(): Reportable { return { test: 42 } }", "test"),
            ("nested", "const value: { inner: Reportable } = { inner: { test: 42 } }", "test"),
            ("array element", "const value: Reportable[] = [{ test: 42 }]", "test"),
            (
                "generic constraint",
                "declare function generic<T extends Reportable>(arg: T): void; generic({ test: 42 })",
                "test",
            ),
            (
                "generator contextual property",
                "const gen = function* () { x({ test: 42 }) }",
                "test",
            ),
            (
                "inferred generator constraint",
                "declare function generic<T extends Reportable>(arg: T): void; const gen = function* () { generic({ test: 42 }) }",
                "test",
            ),
            (
                "nested function constraint",
                "declare function generic<T extends Reportable>(arg: T): void; const gen = function* () { function inner() { generic({ test: 42 }) } }",
                "test",
            ),
            (
                "annotated inner generator constraint",
                "declare function generic<T extends Reportable>(arg: T): void; const gen = function* () { const inner = function* (): Generator<never, void, unknown> { generic({ test: 42 }) } }",
                "test",
            ),
            (
                "annotated generator constraint",
                "declare function generic<T extends Reportable>(arg: T): void; const gen = function* (): Generator<never, void, unknown> { generic({ test: 42 }) }",
                "test",
            ),
            (
                "generator declaration constraint",
                "declare function generic<T extends Reportable>(arg: T): void; function* gen() { generic({ test: 42 }) }",
                "test",
            ),
            (
                "generator method constraint",
                "declare function generic<T extends Reportable>(arg: T): void; class C { *gen() { generic({ test: 42 }) } }",
                "test",
            ),
            (
                "ordinary function constraint",
                "declare function generic<T extends Reportable>(arg: T): void; function gen() { generic({ test: 42 }) }",
                "test",
            ),
            (
                "arrow constraint",
                "declare function generic<T extends Reportable>(arg: T): void; const gen = () => generic({ test: 42 })",
                "test",
            ),
            (
                "nested constraint",
                "declare function generic<T extends { inner: Reportable }>(arg: T): void; generic({ inner: { test: 42 } })",
                "test",
            ),
            (
                "array constraint",
                "declare function generic<T extends Reportable[]>(arg: T): void; generic([{ test: 42 }])",
                "test",
            ),
            (
                "rest constraint",
                "declare function generic<T extends Reportable>(...args: T[]): void; generic({ stable: 1 }, { test: 42 })",
                "test",
            ),
            (
                "constructor constraint",
                "declare class Box<T extends Reportable> { constructor(arg: T) } new Box({ test: 42 })",
                "test",
            ),
            (
                "constraint using inference",
                "declare function generic<A, T extends Partial<A>>(a: A, arg: T): void; declare const reportable: Reportable; generic(reportable, { test: 42 })",
                "test",
            ),
            (
                "constraint without property",
                "declare function generic<T extends { other?: number }>(arg: T): void; generic({ test: 42 })",
                "",
            ),
            (
                "unconstrained alternative",
                "declare function generic<T, U extends Reportable>(arg: T | U): void; generic({ test: 42 })",
                "",
            ),
            (
                "fixed tuple spread",
                "declare function generic<T extends Reportable>(prefix: string, arg: T): void; declare const prefix: [string]; generic(...prefix, { test: 42 })",
                "test",
            ),
            (
                "fixed tuple spreads",
                "declare function generic<T extends Reportable>(a: string, b: number, c: boolean, arg: T): void; declare const ab: readonly [string, number]; declare const c: [boolean]; generic(...ab, ...c, { test: 42 })",
                "test",
            ),
            (
                "return type inference",
                "interface Box<A> { readonly value: A } declare function succeed<A>(a: A): Box<A>; function make(): Box<Reportable> { return succeed({ test: 42 }) }",
                "test",
            ),
            (
                "constrained return type inference",
                "interface Box<A> { readonly value: A } declare function succeed<A extends object>(a: A): Box<A>; const value: Box<Reportable> = succeed({ test: 42 })",
                "test",
            ),
            (
                "callback return inference",
                "declare function map<A, B>(as: A[], f: (a: A) => B): B[]; const value: Reportable[] = map([1], () => ({ test: 42 }))",
                "test",
            ),
            (
                "default type argument",
                "interface Plain { readonly test?: number } declare function withDefault<T extends Plain = Reportable>(arg: T): void; withDefault({ test: 42 })",
                "test",
            ),
            (
                "tagged template constraint",
                "declare function tag<T extends Reportable>(s: TemplateStringsArray, v: T): void; tag`${{ test: 42 }}`",
                "test",
            ),
            (
                "explicit outer type arguments",
                "declare function ref<T>(key: string, o: { make: () => T }): T; declare function constant<A>(a: A): () => A; ref<Reportable>(\"k\", { make: constant({ test: 42 }) })",
                "test",
            ),
            (
                "nested generic call",
                "declare function id<X>(x: X): X; declare function generic<T extends Reportable>(arg: T): void; generic(id({ test: 42 }))",
                "test",
            ),
            (
                "generic own declaration",
                "declare function generic<T>(arg: T): void; generic({\n/** @stability TAG */\ntest: 42 })",
                "",
            ),
            (
                "inherited",
                "interface Child extends Reportable {} const value: Child = { test: 42 }",
                "test",
            ),
            ("property access", "declare function make(): Reportable; make().test", "test"),
            (
                "value reference",
                "/** @stability TAG */\nconst value = 42; x({ stable: value })",
                "value",
            ),
            ("stable property", "x({ stable: 42 })", ""),
            ("omitted property", "x({})", ""),
            ("untyped literal", "const value = { test: 42 }; x(value)", ""),
            ("untyped shorthand", "const test = 42; const value = { test }; x(value)", ""),
            ("spread", "const value = { test: 42 }; x({ ...value })", ""),
            ("different property", "const value: { test?: number } = { test: 42 }", ""),
            (
                "stable union branch",
                r#"type Choice = (Reportable & { kind: "experimental" }) | { kind: "stable"; test?: number }; const value: Choice = { kind: "stable", test: 42 }"#,
                "",
            ),
            (
                "experimental union branch",
                r#"type Choice = (Reportable & { kind: "experimental" }) | { kind: "stable"; test?: number }; const value: Choice = { kind: "experimental", test: 42 }"#,
                "test",
            ),
            (
                "ambiguous union",
                "type Choice = Reportable | { test?: string }; const value: Choice = { test: 42 }",
                "test",
            ),
            (
                "union deduplication",
                r#"type Choice = (Reportable & { kind?: "a" }) | (Reportable & { kind?: "b" }); const value: Choice = { test: 42 }"#,
                "test",
            ),
            (
                "method",
                "/** @stability stable */\ninterface Methods {\n/** @stability TAG */\ntest(): number }; const value: Methods = { test() { return 42 } }",
                "test",
            ),
        ];
        let tagged_rules: [(&str, &Rule); 2] =
            [("experimental", &EXPERIMENTAL_API_USAGE), ("unstable", &UNSTABLE_API_USAGE)];
        for (tag, rule) in tagged_rules {
            for &(name, test_source, want) in tests {
                t.run(&format!("{tag}/{name}"), |t| {
                    t.parallel();
                    let source = format!(
                        "/** @stability stable */
interface Reportable {{
  /** @stability TAG */
  readonly test?: number
  stable?: number
}}
/** @stability stable */
declare function x(arg: Reportable): void
{test_source}"
                    )
                    .replace("TAG", tag);
                    let mut compiled = compile_api_stability_source(t, &source);
                    let diagnostics = compiled.with_rule_ctx(|ctx| (rule.run)(ctx));
                    if want.is_empty() {
                        if !diagnostics.is_empty() {
                            t.fatal(ets_go::sprintf!(
                                "got diagnostics %s, want none",
                                diagnostic_texts(&diagnostics)
                            ));
                        }
                        return;
                    }
                    if diagnostics.len() != 1 {
                        t.fatal(ets_go::sprintf!(
                            "got %d diagnostics, want 1: %s",
                            diagnostics.len(),
                            diagnostic_texts(&diagnostics)
                        ));
                    }
                    let diagnostic = diagnostics[0];
                    let got = diagnostic_text(&source, diagnostic);
                    if got != want {
                        t.error(ets_go::sprintf!("diagnostic highlights %q, want %q", got, want));
                    }
                    if !diagnostic.localize().contains(&format!("effect({})", rule.name)) {
                        t.error(ets_go::sprintf!(
                            "unexpected diagnostic: %s",
                            diagnostic.localize()
                        ));
                    }
                });
            }
        }
    });
}

// go test: internal/rules/stability_api_usage_test.go:166 TestStabilityApiUsageContextualPropertiesAllowlist 8d13a3ba1a1ff6ff
#[test]
fn test_stability_api_usage_contextual_properties_allowlist() {
    run(|t| {
        t.parallel();
        for allowed in ["", "example", "example/api", "example/api#Reportable", "example/api#other"]
        {
            t.run(allowed, |t| {
                t.parallel();
                let mut compiled = compile_api_stability_files(
                    t,
                    &files(&[
                        ("package.json", r#"{"name":"example"}"#),
                        (
                            "api.ts",
                            "/** @stability stable */
export interface Reportable {
  /** @stability experimental */
  readonly test?: number
}",
                        ),
                        (
                            "test.ts",
                            r#"import type { Reportable } from "./api.js"
declare function x(arg: Reportable): void
x({ test: 42 })"#,
                        ),
                    ]),
                );
                compiled.options = Some(ResolvedEffectPluginOptions {
                    allowed_experimental_apis: vec![allowed.to_string()],
                    ..Default::default()
                });
                let diagnostics = compiled.with_rule_ctx(|ctx| (EXPERIMENTAL_API_USAGE.run)(ctx));
                let mut want = 0;
                if allowed.is_empty() || allowed == "example/api#other" {
                    want = 1;
                }
                if diagnostics.len() != want {
                    t.fatal(ets_go::sprintf!(
                        "got %d diagnostics, want %d: %s",
                        diagnostics.len(),
                        want,
                        diagnostic_texts(&diagnostics)
                    ));
                }
                if want == 1 && !diagnostics[0].localize().contains("`example/api#Reportable`") {
                    t.error(ets_go::sprintf!("unexpected API name: %s", diagnostics[0].localize()));
                }
            });
        }
    });
}

// go test: internal/rules/stability_api_usage_test.go:199 TestStabilityApiUsageRepeatedContextualProperties db4ab0a3060eff6e
#[test]
fn test_stability_api_usage_repeated_contextual_properties() {
    run(|t| {
        t.parallel();
        for call in ["accept", "generic"] {
            t.run(call, |t| {
                t.parallel();
                let source = format!(
                    "interface Options {{
  /** @stability experimental */
  first: number
  stable?: number
  /** @stability unstable */
  second: number
  nested?: Options
}}

declare function accept(options: Options): void
declare function generic<T extends Options>(options: T): void
{call}({{ first: 1, stable: 2, second: 3, nested: {{ first: 4, second: 5 }} }})
const unrelated = {{ first: 8, second: 9 }}
accept({{ first: 6, second: 7 }})
"
                );
                let tests: [(&Rule, &str, [&str; 3]); 2] = [
                    (&EXPERIMENTAL_API_USAGE, "first", ["first: 1", "first: 4", "first: 6"]),
                    (&UNSTABLE_API_USAGE, "second", ["second: 3", "second: 5", "second: 7"]),
                ];
                for order in [[0, 1], [1, 0]] {
                    t.run(&format!("{}-first", tests[order[0]].0.name), |t| {
                        t.parallel();
                        let mut compiled = compile_api_stability_source(t, &source);
                        compiled.with_rule_ctx(|ctx| {
                            for index in order {
                                let (rule, name, uses) = &tests[index];
                                let diagnostics = (rule.run)(ctx);
                                if diagnostics.len() != 3 {
                                    t.fatal(ets_go::sprintf!(
                                        "%s: got %d diagnostics, want 3: %s",
                                        rule.name,
                                        diagnostics.len(),
                                        diagnostic_texts(&diagnostics)
                                    ));
                                }
                                for (i, &diagnostic) in diagnostics.iter().enumerate() {
                                    let want =
                                        source.find(uses[i]).expect("the use is in the source");
                                    if diagnostic_pos(diagnostic) != want {
                                        t.error(ets_go::sprintf!(
                                            "%s position = %d, want %d",
                                            rule.name,
                                            diagnostic_pos(diagnostic),
                                            want
                                        ));
                                    }
                                    let got = diagnostic_text(&source, diagnostic);
                                    if got != *name {
                                        t.error(ets_go::sprintf!(
                                            "%s highlights %q, want %q",
                                            rule.name,
                                            got,
                                            *name
                                        ));
                                    }
                                }
                            }
                        });
                    });
                }
            });
        }
    });
}

// go test: internal/rules/stability_api_usage_test.go:257 TestStabilityContextualCacheDiscriminants 8ec680ae18e221d4
// TestStabilityContextualCacheDiscriminants checks that the per-literal
// contextual type cache does not leak one property's lookup into another: the
// same property name resolves to a different union member depending on each
// literal's discriminant, including literals nested inside property values.
#[test]
fn test_stability_contextual_cache_discriminants() {
    run(|t| {
        t.parallel();
        let source = r#"interface A {
  kind: "a"
  /** @stability experimental */
  x: number
  y?: number
}
interface B {
  kind: "b"
  x: number
  /** @stability unstable */
  y?: number
}
declare function pick<T extends A | B>(value: T): T
pick({ kind: "a", x: pick({ kind: "b", x: 1, y: 2 }).x, y: 3 })
pick({ kind: "b", y: pick({ kind: "a", x: 4 }).x, x: 5 })
"#;
        for prime in [true, false] {
            t.run(&format!("prime={prime}"), |t| {
                t.parallel();
                let tests: [(&Rule, [&str; 2]); 2] = [
                    (&EXPERIMENTAL_API_USAGE, ["x: pick", "x: 4"]),
                    (&UNSTABLE_API_USAGE, ["y: 2", "y: pick"]),
                ];
                for order in [[0, 1], [1, 0]] {
                    t.run(&format!("{}-first", tests[order[0]].0.name), |t| {
                        t.parallel();
                        let mut compiled = compile_api_stability_source_primed(t, source, prime);
                        compiled.with_rule_ctx(|ctx| {
                            for index in order {
                                let (rule, uses) = &tests[index];
                                let mut got = Vec::new();
                                let mut want = Vec::new();
                                for diagnostic in (rule.run)(ctx) {
                                    got.push(format!(
                                        "{}:{}",
                                        diagnostic_pos(diagnostic),
                                        diagnostic_text(source, diagnostic)
                                    ));
                                }
                                for usage in uses {
                                    want.push(format!(
                                        "{}:{}",
                                        source.find(usage).expect("the use is in the source"),
                                        &usage[..1]
                                    ));
                                }
                                if got != want {
                                    t.error(ets_go::sprintf!(
                                        "%s: got %s, want %s",
                                        rule.name,
                                        format!("[{}]", got.join(" ")),
                                        format!("[{}]", want.join(" "))
                                    ));
                                }
                            }
                        });
                    });
                }
            });
        }
    });
}
