// Ported from effect-tsgo's internal/rules/api_stability_leak_test.go (helpers in
// api_stability_leak_regression_test.rs).

use ets_go::testing::T;
use ets_shim::core::context::Context as GoContext;
use ets_typeparser::get_symbol_of_declaration;

use crate::api_stability_leak_regression_test::*;
use crate::*;

// go test: internal/rules/api_stability_leak_test.go:24 compileApiStabilitySource ded003f532cebd39
pub(crate) fn compile_api_stability_source(t: &T, source: &str) -> ApiStabilityCompiled {
    t.helper();
    compile_api_stability_source_primed(t, source, true)
}

// go test: internal/rules/api_stability_leak_test.go:31 compileApiStabilitySourcePrimed f2dbe99bc8744346
// compileApiStabilitySourcePrimed optionally skips the full type-check, so the
// rules run against a cold checker.
//
// Rust: the same program as `compile_api_stability_files_primed` over the one file `/.src/test.ts`.
pub(crate) fn compile_api_stability_source_primed(
    t: &T,
    source: &str,
    prime: bool,
) -> ApiStabilityCompiled {
    t.helper();
    compile_api_stability_files_primed(t, &files(&[("test.ts", source)]), prime)
}

// go test: internal/rules/api_stability_leak_test.go:72 TestApiStabilityLeakRule 31af6d6a5b430783
#[test]
fn test_api_stability_leak_rule() {
    run(|t| {
        t.parallel();

        let source = r"/** @stability unstable */
export interface Unstable { value: string }
/** @stability experimental */
export interface Experimental { value: string }
export interface Container<T> { value: T }
export interface StableLeak { member: Unstable }
export interface StableOk { member: string }
export type StableGenericLeak = Container<Experimental>
export class PrivateLeak {
  private secret: Experimental
  protected guarded: Unstable
  public exposed: Unstable
}
/** @stability unstable */
export interface UnstableLeak { member: Experimental }
/** @stability unstable */
export interface UnstableOk { member: Unstable }
/** @stability experimental */
export interface ExperimentalOk { member: Unstable }
";

        let mut compiled = compile_api_stability_source(t, source);
        compiled.with_rule_ctx(|rule_ctx| {
            let diagnostics = (API_STABILITY_LEAK.run)(rule_ctx);
            let mut messages = Vec::new();
            for diagnostic in &diagnostics {
                messages.push(diagnostic.localize());
            }
            if diagnostics.len() != 4 {
                t.fatal(format!("got {} diagnostics, want 4: {messages:?}", diagnostics.len()));
            }
            let contains = |want: &str| messages.iter().any(|message| message.contains(want));
            for want in [
                "`StableLeak` exposes `Unstable`, an unstable API.",
                "`StableGenericLeak` exposes `Experimental`, an experimental API.",
                "`UnstableLeak` exposes `Experimental`, an experimental API.",
                "`PrivateLeak` exposes `Unstable`, an unstable API.",
            ] {
                if !contains(want) {
                    t.error(format!("missing diagnostic {want:?} in {messages:?}"));
                }
            }
            if contains("`PrivateLeak` exposes `Experimental`") {
                t.error(format!("private member leaked into the public surface: {messages:?}"));
            }
        });
    });
}

// go test: internal/rules/api_stability_leak_test.go:130 TestApiStabilityLeakComputedNames af35d488de3b1dcf
// TestApiStabilityLeakComputedNames pins the production diagnostic path
// that previously panicked on a type literal with a computed member name. The
// rule must report the erased experimental aliases written in the computed
// property and method annotations without evaluating the computed name, and it
// must not change the compiler's own diagnostics.
#[test]
fn test_api_stability_leak_computed_names() {
    run(|t| {
        t.parallel();

        let mut compiled = compile_api_stability_files_primed(
            t,
            &files(&[(
                "test.ts",
                r#"/** @stability experimental */
type PropertyAlias = string
/** @stability experimental */
type ParameterAlias = string
/** @stability experimental */
type MethodAlias = string
export declare const x: {
  [Symbol.iterator]?: PropertyAlias
  [Symbol.toStringTag](value: ParameterAlias): MethodAlias
  ["computed"]: PropertyAlias
}
export type Y = { readonly [Symbol.iterator]?: never }
export interface Z { readonly [Symbol.iterator]?: never }
"#,
            )]),
            true,
        );
        compiled.with_rule_ctx(|rule_ctx| {
            let before = api_stability_compiler_diagnostics(rule_ctx);
            let before_instantiation = rule_ctx.checker().total_instantiation_count;
            let messages = run_api_stability_messages(t, rule_ctx);
            let after = api_stability_compiler_diagnostics(rule_ctx);
            if before != after {
                t.error(format!("rule changed compiler diagnostics:\nbefore={before:?}\nafter={after:?}"));
            }
            let source_file = rule_ctx.source_file;
            for diagnostic in
                rule_ctx.checker().get_diagnostics_exported(&GoContext::background(), source_file)
            {
                if diagnostic.code() == 2589 {
                    t.error(format!("rule introduced TS2589: {}", diagnostic.localize()));
                }
            }
            let delta = rule_ctx.checker().total_instantiation_count - before_instantiation;
            if delta > API_STABILITY_MAX_INSTANTIATION_DELTA {
                t.error(format!(
                    "rule instantiated {delta} types, want at most {API_STABILITY_MAX_INSTANTIATION_DELTA}"
                ));
            }
            for want in
                ["`x` exposes `PropertyAlias`", "`x` exposes `ParameterAlias`", "`x` exposes `MethodAlias`"]
            {
                if !contains_api_stability_message(t, &messages, want) {
                    t.error(format!("missing {want:?} in {messages:?}"));
                }
            }
        });
    });
}

// go test: internal/rules/api_stability_leak_test.go:181 TestApiStabilityLeakExportOrderIsDeterministic 2c5b05da353f6e29
// TestApiStabilityLeakExportOrderIsDeterministic pins the deterministic
// export visit order the rule and the namespace walker use. The checker returns
// module exports in Go map order, and the order in which the per-checker
// dependency caches are filled decides which surfaces a later analysis reuses;
// an unsorted visit therefore makes the reported findings vary between runs of
// the same binary over the same sources. The helper must keep returning the
// same name-ordered sequence and must never drop an export.
//
// Rust: tsrs's exports hold no nil, so Go's nil-symbol checks cannot fail and drop.
#[test]
fn test_api_stability_leak_export_order_is_deterministic() {
    run(|t| {
        t.parallel();

        let mut compiled = compile_api_stability_files_primed(
            t,
            &files(&[(
                "test.ts",
                r"/** @stability experimental */
interface E { value: string }
/** @stability experimental */
type P = string
export declare const zeta: E
export declare const alpha: P
export interface Middle { value: E }
export declare const beta: { value: P }
export declare const omega: Middle
",
            )]),
            true,
        );
        compiled.with_rule_ctx(|rule_ctx| {
            let source_file = rule_ctx.source_file;
            let Some(module) = get_symbol_of_declaration(rule_ctx.checker(), source_file.as_node())
            else {
                t.fatal("module symbol missing");
            };
            let all = rule_ctx.checker().get_exports_of_module_exported(module);
            if all.len() < 5 {
                t.fatal(format!("fixture should export several symbols, got {}", all.len()));
            }
            let mut want_names: Vec<&str> = all.iter().map(|symbol| symbol.name()).collect();
            want_names.sort_unstable();

            let mut first: Option<Vec<&str>> = None;
            for _ in 0..64 {
                let ordered = sorted_api_stability_exports(rule_ctx, module);
                if ordered.len() != all.len() {
                    t.fatal(format!(
                        "export sorting dropped symbols: got {}, want {}",
                        ordered.len(),
                        all.len()
                    ));
                }
                let names: Vec<&str> = ordered.iter().map(|symbol| symbol.name()).collect();
                if !names.is_sorted() {
                    t.fatal(format!("export order is not canonical: {names:?}"));
                }
                let Some(first) = &first else {
                    first = Some(names);
                    continue;
                };
                if *first != names {
                    t.fatal(format!(
                        "export order changed between invocations:\nfirst={first:?}\nlater={names:?}"
                    ));
                }
            }
            if first.as_ref() != Some(&want_names) {
                t.fatal(format!(
                    "canonical order does not match the sorted export set:\ngot={first:?}\nwant={want_names:?}"
                ));
            }
        });
    });
}

// go test: internal/rules/api_stability_leak_test.go:246 TestApiStabilityLeakInternalExportsIgnored de00527036181ef9
// TestApiStabilityLeakInternalExportsIgnored pins the export-level
// `@internal` exclusion: an export whose own declaration carries the tag is not
// checked, while its public peers still are. The tag is read from the parsed
// JSDoc (the same marker native TypeScript uses for `stripInternal` declaration
// emit), never from a raw substring of the surrounding prose.
#[test]
fn test_api_stability_leak_internal_exports_ignored() {
    run(|t| {
        t.parallel();

        let experimental_e = r"/** @stability experimental */
interface E { value: string }
declare const e: E
"
        .to_string();
        let tests = vec![
            case(
                "direct internal export is ignored while its public peer warns",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"/** @internal */
export declare const hidden: E
export declare const shown: E
",
                    ]),
                )]),
                &["`shown` exposes `E`"],
                &["`hidden` exposes"],
            ),
            case(
                "declaration attachment across declaration kinds",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"/** @internal */
export const hiddenVar: E = e
/** @internal */
export declare function hiddenFunction(): E
/** @internal */
export interface HiddenInterface { value: E }
/** @internal */
export type HiddenType = { value: E }
/** @internal */
export declare class HiddenClass { value: E }
export declare const shown: E
",
                    ]),
                )]),
                &["`shown` exposes `E`"],
                &[
                    "`hiddenVar` exposes",
                    "`hiddenFunction` exposes",
                    "`HiddenInterface` exposes",
                    "`HiddenType` exposes",
                    "`HiddenClass` exposes",
                ],
            ),
            case(
                "default function export attachment",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
interface E { value: string }
/** @internal */
export default function hiddenDefault(): E {
  return undefined as unknown as E
}
",
                )]),
                &[],
                &["`default` exposes"],
            ),
            case(
                "default assignment export attachment",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"/** @internal */
export default e
",
                    ]),
                )]),
                &[],
                &["`default` exposes"],
            ),
            case(
                "named forwarding tag on the declaration itself",
                files(&[
                    (
                        "test.ts",
                        r#"/** @internal */
export { Leaky } from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            r"export interface Leaky { value: E }
",
                        ]),
                    ),
                ]),
                &[],
                &["`Leaky` exposes"],
            ),
            case(
                "untagged named forwarding of an internal target is skipped",
                files(&[
                    (
                        "test.ts",
                        r#"export { Hidden } from "./dep"
export { Shown } from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            r"/** @internal */
export interface Hidden { value: E }
export interface Shown { value: E }
",
                        ]),
                    ),
                ]),
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            case(
                "named forwarding tag covers every specifier in the declaration",
                files(&[
                    (
                        "test.ts",
                        r#"/** @internal */
export { LeakyOne, LeakyTwo } from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            r"export interface LeakyOne { value: E }
export interface LeakyTwo { value: E }
",
                        ]),
                    ),
                ]),
                &[],
                &["`LeakyOne` exposes", "`LeakyTwo` exposes"],
            ),
            case(
                "specifier tag ignores only that specifier",
                files(&[
                    (
                        "test.ts",
                        r#"export {
  /** @internal */
  Hidden,
  Shown
} from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            r"export interface Hidden { value: E }
export interface Shown { value: E }
",
                        ]),
                    ),
                ]),
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            case(
                "star forwarding tag on the declaration itself",
                files(&[
                    (
                        "test.ts",
                        r#"/** @internal */
export * from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            r"export interface Leaky { value: E }
",
                        ]),
                    ),
                ]),
                &[],
                &["`Leaky` exposes"],
            ),
            case(
                "untagged star forwarding of an internal target is skipped",
                files(&[
                    (
                        "test.ts",
                        r#"export * from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            r"/** @internal */
export interface Hidden { value: E }
export interface Shown { value: E }
",
                        ]),
                    ),
                ]),
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            case(
                "namespace forwarding tag on the declaration itself",
                files(&[
                    (
                        "test.ts",
                        r#"/** @internal */
export * as api from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            r"export interface Leaky { value: E }
",
                        ]),
                    ),
                ]),
                &[],
                &["`api` exposes"],
            ),
            case(
                "namespace nested internal members are not walked",
                files(&[
                    (
                        "test.ts",
                        r#"export * as api from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
interface E { value: string }
/** @stability experimental */
interface F { value: string }
/** @internal */
export interface Hidden { value: E }
/** @internal @stability experimental */
export interface HiddenTagged { value: string }
export interface Shown { value: F }
",
                    ),
                ]),
                &["`api` exposes `F`"],
                &["`api` exposes `E`", "`api` exposes `HiddenTagged`"],
            ),
            case(
                "alias chain reaching an internal target is skipped",
                files(&[
                    ("test.ts", r#"export { Leaky } from "./barrel""#),
                    ("barrel.ts", r#"export { Leaky } from "./dep""#),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Leaky { value: E }\n",
                        ]),
                    ),
                ]),
                &[],
                &["`Leaky` exposes"],
            ),
            case(
                "internal tag on an intermediate forwarding declaration",
                files(&[
                    ("test.ts", r#"export { Leaky } from "./barrel""#),
                    ("barrel.ts", "/** @internal */\nexport { Leaky } from \"./dep\""),
                    ("dep.ts", &cat(&[&experimental_e, "export interface Leaky { value: E }\n"])),
                ]),
                &[],
                &["`Leaky` exposes"],
            ),
            case(
                "public overload stays checked when only the implementation is internal",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"export function overloaded(x: E): E
export function overloaded(x: string): string
/** @internal */
export function overloaded(x: E | string): E | string {
  return x as E
}
",
                    ]),
                )]),
                &["`overloaded` exposes `E`"],
                &[],
            ),
            case(
                "all overload declarations internal are skipped",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"/** @internal */
export declare function gated(x: E): E
/** @internal */
export declare function gated(x: string): string
",
                    ]),
                )]),
                &[],
                &["`gated` exposes"],
            ),
            case(
                "merged declarations stay public when one declaration is not internal",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"/** @internal */
export interface Merged { a: E }
export interface Merged { b: string }
",
                    ]),
                )]),
                &["`Merged` exposes `E`"],
                &[],
            ),
            case(
                "merged declarations all internal are skipped",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"/** @internal */
export interface MergedHidden { a: E }
/** @internal */
export interface MergedHidden { b: string }
",
                    ]),
                )]),
                &[],
                &["`MergedHidden` exposes"],
            ),
            case(
                "prose mentioning internalized or another tag is not a tag",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"/** This representation is internalized and @internally tracked. */
export interface Prose { value: E }
",
                    ]),
                )]),
                &["`Prose` exposes `E`"],
                &[],
            ),
            case(
                "public signature exposing an internal unstable type still warns",
                files(&[(
                    "test.ts",
                    r"/** @internal @stability experimental */
interface InternalE { value: string }
/** @internal @stability unstable */
export interface InternalU { value: string }
export interface PublicSurface { value: InternalE }
export interface PublicOther { value: InternalU }
",
                )]),
                &[
                    "`PublicSurface` exposes `InternalE`, an experimental API.",
                    "`PublicOther` exposes `InternalU`, an unstable API.",
                ],
                &[],
            ),
            case(
                "internal export with a redundant cyclic star path stays skipped",
                files(&[
                    (
                        "test.ts",
                        &cat(&[
                            &experimental_e,
                            r#"/** @internal */
export interface Cyclic { value: E }
export * from "./dep"
"#,
                        ]),
                    ),
                    (
                        "dep.ts",
                        r#"export * from "./test"
"#,
                    ),
                ]),
                &[],
                &["`Cyclic` exposes"],
            ),
            case(
                "public star declaration behind a cycle stays checked",
                files(&[
                    (
                        "test.ts",
                        r#"export * from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        r#"export * from "./test"
/** @stability experimental */
interface E { value: string }
export interface CyclicPublic { value: E }
"#,
                    ),
                ]),
                &["`CyclicPublic` exposes `E`"],
                &[],
            ),
            case(
                "internal export visited first does not poison its public peer",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"/** @internal */
export declare const aHidden: E
export declare const zShown: E
",
                    ]),
                )]),
                &["`zShown` exposes `E`"],
                &["`aHidden` exposes"],
            ),
            case(
                "internal export visited last does not poison its public peer",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"export declare const aShown: E
/** @internal */
export declare const zHidden: E
",
                    ]),
                )]),
                &["`aShown` exposes `E`"],
                &["`zHidden` exposes"],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_test.go:548 TestApiStabilityLeakExplicitExportPrecedence 897714d5d8c14b6a
// TestApiStabilityLeakExplicitExportPrecedence pins that:
// an explicit `@internal` export is the compiler's export winner for its name,
// so a redundant `export *` forward of the same name must not defeat the tag.
// Tagged statements, tagged specifiers, alias chains and cyclic redundant star
// paths all preserve the tag, while public explicit exports stay checked even
// when a redundant star forward is tagged internal.
#[test]
fn test_api_stability_leak_explicit_export_precedence() {
    run(|t| {
        t.parallel();

        let experimental_e = r"/** @stability experimental */
interface E { value: string }
"
        .to_string();
        let leaky_dep = cat(&[&experimental_e, "export interface Leaky { value: E }\n"]);
        let tests = vec![
            // Reviewer reproduction: the tagged named export governs the name; the
            // redundant star forward resolves to the same public target but must
            // not re-enable the export.
            case(
                "tagged named export shadows redundant public star",
                files(&[
                    (
                        "test.ts",
                        "/** @internal */\nexport { Leaky } from \"./dep\"\nexport * from \"./dep\"\n",
                    ),
                    ("dep.ts", &leaky_dep),
                ]),
                &[],
                &["`Leaky` exposes"],
            ),
            // Reviewer reproduction: a tagged local declaration is the explicit
            // winner and a cyclic star forward back through a barrel cannot
            // loosen it.
            case(
                "tagged local export shadows redundant public star",
                files(&[
                    (
                        "test.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Leaky { value: E }\nexport * from \"./dep\"\n",
                        ]),
                    ),
                    ("dep.ts", "export { Leaky } from \"./test\"\n"),
                ]),
                &[],
                &["`Leaky` exposes"],
            ),
            // A tag on the specifier itself governs, like a tag above the whole
            // export declaration.
            case(
                "tagged specifier shadows redundant public star",
                files(&[
                    (
                        "test.ts",
                        "export {\n  /** @internal */\n  Leaky\n} from \"./dep\"\nexport * from \"./dep\"\n",
                    ),
                    ("dep.ts", &leaky_dep),
                ]),
                &[],
                &["`Leaky` exposes"],
            ),
            // The explicit path is tagged while the redundant star path reaches
            // the same name through a public barrel; only the explicit export is
            // selected, so the star path stays irrelevant.
            case(
                "tagged explicit export shadows redundant star alias chain",
                files(&[
                    (
                        "test.ts",
                        "/** @internal */\nexport { Leaky } from \"./dep\"\nexport * from \"./other\"\n",
                    ),
                    ("other.ts", "export * from \"./dep\"\n"),
                    ("dep.ts", &leaky_dep),
                ]),
                &[],
                &["`Leaky` exposes"],
            ),
            // A public explicit declaration stays checked even when a redundant
            // star forward of the same name is tagged internal: the explicit
            // declaration is public, so the export is public.
            case(
                "public local export wins over redundant tagged star",
                files(&[
                    (
                        "test.ts",
                        &cat(&[
                            &experimental_e,
                            "export interface Leaky { value: E }\n/** @internal */\nexport * from \"./dep\"\n",
                        ]),
                    ),
                    ("dep.ts", "export * from \"./test\"\n"),
                ]),
                &["`Leaky` exposes `E`"],
                &[],
            ),
            // A public explicit named forward stays checked even when a second
            // redundant tag claims the same name from another module.
            case(
                "public explicit forward wins over redundant tagged star",
                files(&[
                    (
                        "test.ts",
                        "export { Leaky } from \"./dep\"\n/** @internal */\nexport * from \"./other\"\n",
                    ),
                    ("other.ts", "export * from \"./dep\"\n"),
                    ("dep.ts", &leaky_dep),
                ]),
                &["`Leaky` exposes `E`"],
                &[],
            ),
            // An explicitly tagged local declaration with an untagged redundant
            // star through another module stays internal as well.
            case(
                "tagged local export shadows redundant untagged star chain",
                files(&[
                    (
                        "test.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Leaky { value: E }\nexport * from \"./other\"\n",
                        ]),
                    ),
                    ("other.ts", "export * from \"./dep\"\n"),
                    ("dep.ts", "export { Leaky } from \"./test\"\n"),
                ]),
                &[],
                &["`Leaky` exposes"],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_test.go:650 TestApiStabilityLeakDeclaredNamespaceMembers 5f1f814b60e04a61
// TestApiStabilityLeakDeclaredNamespaceMembers pins that:
// symbols declared by a namespace ModuleDeclaration filter their own
// `@internal` export members exactly like source-file namespace re-exports. A
// public member of the same namespace is still walked, and the namespace itself
// remains public.
#[test]
fn test_api_stability_leak_declared_namespace_members() {
    run(|t| {
        t.parallel();

        let experimental = r"/** @stability experimental */
interface E { value: string }
/** @stability experimental */
interface F { value: string }
"
        .to_string();
        let tests = vec![
            // Reviewer reproduction: the tagged member is not part of the
            // namespace's public API, so it cannot leak its dependency.
            case(
                "declared namespace filters tagged member",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental,
                        "export namespace Api {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: string }\n}\n",
                    ]),
                )]),
                &[],
                &["`Api` exposes `E`"],
            ),
            // The namespace stays public and its untagged member is still
            // walked, so filtering does not blanket-suppress the export.
            case(
                "declared namespace checks public peer",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental,
                        "export namespace Api {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: F }\n}\n",
                    ]),
                )]),
                &["`Api` exposes `F`"],
                &["`Api` exposes `E`"],
            ),
            // A public safe constant keeps the namespace public while the
            // internal leaking member does not taint it.
            case(
                "declared namespace public constant with tagged member",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental,
                        "export namespace Api {\n  export const safe = \"ok\"\n  /** @internal */\n  export interface Hidden { value: E }\n}\n",
                    ]),
                )]),
                &[],
                &["`Api` exposes `E`"],
            ),
            // Merged namespace declarations contribute the same export table, so
            // a tag in one block filters the merged member.
            case(
                "merged namespace declarations filter tagged member",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental,
                        "export namespace Api {\n  /** @internal */\n  export interface Hidden { value: E }\n}\nexport namespace Api {\n  export interface Shown { value: F }\n}\n",
                    ]),
                )]),
                &["`Api` exposes `F`"],
                &["`Api` exposes `E`"],
            ),
            // Dotted namespace declarations nest module declarations; the inner
            // module's members are filtered the same way.
            case(
                "dotted namespace filters tagged member",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental,
                        "export namespace Outer.Inner {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: F }\n}\n",
                    ]),
                )]),
                &["`Outer` exposes `F`"],
                &["`Outer` exposes `E`"],
            ),
            // An ambient namespace is a ModuleDeclaration too; its tagged member
            // is filtered while the public peer still warns.
            case(
                "ambient namespace filters tagged member",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental,
                        "declare namespace Ambient {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: F }\n}\nexport { Ambient }\n",
                    ]),
                )]),
                &["`Ambient` exposes `F`"],
                &["`Ambient` exposes `E`"],
            ),
            // Reviewer reproduction for forwarding: a module that re-exports a
            // namespace is walked through the namespace so its tagged member is
            // filtered.
            case(
                "forwarded declared namespace filters tagged member",
                files(&[
                    ("test.ts", "export * as api from \"./dep\"\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental,
                            "export namespace Nested {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: F }\n}\n",
                        ]),
                    ),
                ]),
                &["`api` exposes `F`"],
                &["`api` exposes `E`"],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_test.go:747 TestApiStabilityLeakImportThenExportAliases 2be9903dac9a91a8
// TestApiStabilityLeakImportThenExportAliases pins that:
// an untagged import followed by a local export follows the named or default
// import target to its original declaration. A public target still warns, a
// merged target with a public declaration is not suppressed, and the import
// form used is the one written in the declaration: no other identifier is
// inferred and no type is read.
#[test]
fn test_api_stability_leak_import_then_export_aliases() {
    run(|t| {
        t.parallel();

        let experimental_e = r"/** @stability experimental */
interface E { value: string }
declare const e: E
"
        .to_string();
        let tests = vec![
            // Reviewer reproduction: the named import target is tagged internal,
            // so the local re-export is internal too.
            case(
                "named import then export follows internal target",
                files(&[
                    ("test.ts", "import { Hidden } from \"./dep\"\nexport { Hidden }\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Hidden { value: E }\n",
                        ]),
                    ),
                ]),
                &[],
                &["`Hidden` exposes"],
            ),
            // Reviewer reproduction: a default import followed by export default
            // follows the ImportClause to the tagged original declaration.
            case(
                "default import then export default follows internal target",
                files(&[
                    ("test.ts", "import Hidden from \"./dep\"\nexport default Hidden\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport default function hidden(): E { return null as unknown as E }\n",
                        ]),
                    ),
                ]),
                &[],
                &["`default` exposes"],
            ),
            case(
                "renamed import then export follows internal target",
                files(&[
                    ("test.ts", "import { Hidden as Alias } from \"./dep\"\nexport { Alias }\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Hidden { value: E }\n",
                        ]),
                    ),
                ]),
                &[],
                &["`Alias` exposes"],
            ),
            case(
                "named import then export default follows internal target",
                files(&[
                    ("test.ts", "import { Hidden } from \"./dep\"\nexport default Hidden\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport function Hidden(): E { return null as unknown as E }\n",
                        ]),
                    ),
                ]),
                &[],
                &["`default` exposes"],
            ),
            case(
                "import then export through barrel follows internal target",
                files(&[
                    ("test.ts", "import { Hidden } from \"./barrel\"\nexport { Hidden }\n"),
                    ("barrel.ts", "export { Hidden } from \"./dep\"\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Hidden { value: E }\n",
                        ]),
                    ),
                ]),
                &[],
                &["`Hidden` exposes"],
            ),
            // The imported target is an untagged re-export of an internal local
            // declaration; following the import target chain keeps it internal.
            case(
                "import then export follows untagged target re-export",
                files(&[
                    ("test.ts", "import { Hidden } from \"./dep\"\nexport { Hidden }\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\ninterface Hidden { value: E }\nexport { Hidden }\n",
                        ]),
                    ),
                ]),
                &[],
                &["`Hidden` exposes"],
            ),
            // Direct public import and export still warns: only the imported
            // declaration's tag is inherited.
            case(
                "public named import then export still warns",
                files(&[
                    ("test.ts", "import { Shown } from \"./dep\"\nexport { Shown }\n"),
                    ("dep.ts", &cat(&[&experimental_e, "export interface Shown { value: E }\n"])),
                ]),
                &["`Shown` exposes `E`"],
                &[],
            ),
            case(
                "public default import then export default still warns",
                files(&[
                    ("test.ts", "import Shown from \"./dep\"\nexport default Shown\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "export default function shown(): E { return null as unknown as E }\n",
                        ]),
                    ),
                ]),
                &["`default` exposes `E`"],
                &[],
            ),
            // A merged target with a public declaration is public, so the import
            // re-export is not suppressed by the tagged declaration.
            case(
                "merged import target with public declaration stays checked",
                files(&[
                    ("test.ts", "import { Merged } from \"./dep\"\nexport { Merged }\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Merged { a: E }\nexport interface Merged { b: string }\n",
                        ]),
                    ),
                ]),
                &["`Merged` exposes `E`"],
                &[],
            ),
            // A namespace import targets the module itself, not a tagged
            // declaration; its internal members are filtered like any namespace
            // re-export, so no warning is reported.
            case(
                "namespace import then export filters internal member",
                files(&[
                    ("test.ts", "import * as ns from \"./dep\"\nexport { ns }\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Hidden { value: E }\n",
                        ]),
                    ),
                ]),
                &[],
                &["`ns` exposes"],
            ),
            // An explicit tag on the local export statement still governs over
            // the followed import target.
            case(
                "import then tagged export statement stays internal",
                files(&[
                    (
                        "test.ts",
                        "import { Shown } from \"./dep\"\n/** @internal */\nexport { Shown }\n",
                    ),
                    ("dep.ts", &cat(&[&experimental_e, "export interface Shown { value: E }\n"])),
                ]),
                &[],
                &["`Shown` exposes"],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_test.go:875 TestApiStabilityLeakExplicitStableTag d6b44ffb3f86221a
// TestApiStabilityLeakExplicitStableTag pins the branch-facing explicit
// stable support at the leak-rule level: `@stability stable` is a real tag, so
// a stable root still enforces its ceiling, and an explicit stable forwarding
// tag tightens a forwarded experimental symbol (it is not treated as absent).
#[test]
fn test_api_stability_leak_explicit_stable_tag() {
    run(|t| {
        t.parallel();

        let tests = vec![
            case(
                "explicit stable root still enforces its ceiling",
                single_api_stability_file("/** @stability stable */\nexport declare const root: E"),
                &["`root` exposes `E`"],
                &[],
            ),
            case(
                "explicit stable forwarding overrides inherited experimental",
                files(&[
                    ("test.ts", "/** @stability stable */\nexport { Leaky } from \"./dep\"\n"),
                    (
                        "dep.ts",
                        "/** @stability experimental */\ninterface E { value: string }\n/** @stability experimental */\nexport interface Leaky { value: E }\n",
                    ),
                ]),
                &["`Leaky` exposes `E`"],
                &[],
            ),
            case(
                "explicit stable forwarding overrides inherited unstable",
                files(&[
                    ("test.ts", "/** @stability stable */\nexport { Leaky } from \"./dep\"\n"),
                    (
                        "dep.ts",
                        "/** @stability experimental */\ninterface E { value: string }\n/** @stability unstable */\nexport interface Leaky { value: E }\n",
                    ),
                ]),
                &["`Leaky` exposes `E`"],
                &[],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_test.go:917 TestApiStabilityLeakVisibilityIsMetadataOnly b14b5694ef96b25d
// TestApiStabilityLeakVisibilityIsMetadataOnly pins the metadata boundary
// of the export-level visibility work: filtering an internal export never
// resolves a type, so it adds no checker instantiation and produces no global
// diagnostic. The public peer is still analyzed, and a repeated run over the
// same checker is identical, so the metadata decisions cannot poison a later
// analysis.
#[test]
fn test_api_stability_leak_visibility_is_metadata_only() {
    run(|t| {
        t.parallel();

        let files = files(&[(
            "test.ts",
            r"/** @stability experimental */
interface E { value: string }
declare const e: E
/** @internal */
export declare const hidden: E
export declare const shown: E
",
        )]);

        let mut compiled = compile_api_stability_files(t, &files);
        compiled.with_rule_ctx(|rule_ctx| {
            let source_file = rule_ctx.source_file;
            if get_symbol_of_declaration(rule_ctx.checker(), source_file.as_node()).is_none() {
                t.fatal("missing module symbol");
            }

            let before = rule_ctx.checker().total_instantiation_count;
            let first = run_api_stability_messages(t, rule_ctx);
            let delta = rule_ctx.checker().total_instantiation_count - before;
            let second = run_api_stability_messages(t, rule_ctx);
            let delta2 = rule_ctx.checker().total_instantiation_count - before - delta;
            let globals = rule_ctx.checker().get_global_diagnostics();

            if delta != 0 {
                t.error(format!("visibility filtering instantiated {delta} types, want 0"));
            }
            if delta2 != 0 {
                t.error(format!("repeated run instantiated {delta2} types, want 0"));
            }
            if !globals.is_empty() {
                let globals: Vec<String> =
                    globals.iter().map(|diagnostic| diagnostic.localize()).collect();
                t.error(format!("visibility filtering produced global diagnostics: {globals:?}"));
            }
            if first != second {
                t.error(format!("repeated runs disagree:\nfirst={first:?}\nsecond={second:?}"));
            }
            if contains_api_stability_message(t, &first, "`hidden` exposes") {
                t.error(format!("tagged export was checked: {first:?}"));
            }
            if !contains_api_stability_message(t, &first, "`shown` exposes `E`") {
                t.error(format!("public peer lost its diagnostic: {first:?}"));
            }
        });
    });
}

// go test: internal/rules/api_stability_leak_test.go:969 TestApiStabilityLeakExportVisibilityIsOrderIndependent 313b9ad2741edef0
// TestApiStabilityLeakExportVisibilityIsOrderIndependent runs the
// explicit-precedence, declared-namespace and import-alias fixtures in both
// declaration orders and requires the same diagnostics, so the corrected export
// selection cannot depend on the order in which exports are analyzed.
#[test]
fn test_api_stability_leak_export_visibility_is_order_independent() {
    run(|t| {
        t.parallel();

        let experimental_e = "/** @stability experimental */\ninterface E { value: string }\n";

        let orders = [
            (
                "internal first",
                files(&[
                    (
                        "test.ts",
                        &cat(&[
                            experimental_e,
                            "/** @internal */\nexport interface Hidden { value: E }\nexport interface Shown { value: E }\nexport * from \"./dep\"\n",
                        ]),
                    ),
                    ("dep.ts", "export * from \"./test\"\n"),
                ]),
            ),
            (
                "public first",
                files(&[
                    (
                        "test.ts",
                        &cat(&[
                            experimental_e,
                            "export interface Shown { value: E }\n/** @internal */\nexport interface Hidden { value: E }\nexport * from \"./dep\"\n",
                        ]),
                    ),
                    ("dep.ts", "export * from \"./test\"\n"),
                ]),
            ),
        ];

        for (name, files) in &orders {
            t.run(name, |t| {
                t.parallel();
                assert_api_stability_sources_parse(t, files);
                let mut compiled = compile_api_stability_files(t, files);
                compiled.with_rule_ctx(|rule_ctx| {
                    let messages = run_api_stability_messages(t, rule_ctx);
                    if !contains_api_stability_message(t, &messages, "`Shown` exposes `E`") {
                        t.error(format!("public export lost its diagnostic: {messages:?}"));
                    }
                    if contains_api_stability_message(t, &messages, "`Hidden` exposes") {
                        t.error(format!("tagged export was checked: {messages:?}"));
                    }
                });
            });
        }
    });
}

// go test: internal/rules/api_stability_leak_test.go:1017 TestApiStabilityLeakImportEqualsAliases 1f5827a124aec072
// TestApiStabilityLeakImportEqualsAliases pins that an
// `export import X = ...` alias follows the target named by its module
// reference exactly like a named or default import alias, so an untagged
// forwarding of an `@internal` target is not checked while a public target is,
// including when the target is reached through a namespace qualifier, a typed
// alias chain or an external `require`.
#[test]
fn test_api_stability_leak_import_equals_aliases() {
    run(|t| {
        t.parallel();

        let experimental_e = r"/** @stability experimental */
interface E { value: string }
"
        .to_string();
        let namespace_impl = cat(&[
            &experimental_e,
            "namespace Impl {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: E }\n}\n",
        ]);
        let tests = vec![
            // Reviewer reproduction: the local namespace qualifier names the
            // tagged interface, so only the untagged peer stays checked.
            case(
                "local namespace import equals follows internal target",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &namespace_impl,
                        "export import Hidden = Impl.Hidden\nexport import Shown = Impl.Shown\n",
                    ]),
                )]),
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            // A tag written on the `export import` declaration itself governs
            // over the forwarded symbol, including when the target is public.
            case(
                "tagged import equals declaration stays internal",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &namespace_impl,
                        "/** @internal */\nexport import Hidden = Impl.Shown\n",
                    ]),
                )]),
                &[],
                &["`Hidden` exposes"],
            ),
            // A named import of the namespace resolves the qualifier to the
            // namespace declared in the other file.
            case(
                "cross file namespace import equals follows internal target",
                files(&[
                    (
                        "test.ts",
                        "import { Impl } from \"./dep\"\nexport import Hidden = Impl.Hidden\nexport import Shown = Impl.Shown\n",
                    ),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "export namespace Impl {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: E }\n}\n",
                        ]),
                    ),
                ]),
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            // A namespace import names the module; the target is read from the
            // module's export table.
            case(
                "namespace import equals follows internal member",
                files(&[
                    (
                        "test.ts",
                        "import * as Dep from \"./dep\"\nexport import Hidden = Dep.Hidden\nexport import Shown = Dep.Shown\n",
                    ),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Hidden { value: E }\nexport interface Shown { value: E }\n",
                        ]),
                    ),
                ]),
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            // A dotted namespace declaration nests its module; the innermost
            // qualifier is the module the target is filtered from.
            case(
                "dotted namespace import equals follows internal target",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        "export namespace Outer.Inner {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: E }\n}\nexport import Hidden = Outer.Inner.Hidden\nexport import Shown = Outer.Inner.Shown\n",
                    ]),
                )]),
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            // A local namespace alias chain keeps resolving through metadata:
            // the second alias names the same tagged target.
            case(
                "import equals alias chain follows internal target",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &namespace_impl,
                        "import Alias = Impl\nexport import Hidden = Alias.Hidden\nexport import Shown = Alias.Shown\n",
                    ]),
                )]),
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            // An external `require` alias exposes a member of the required
            // module's export table.
            case(
                "external require import equals follows internal member",
                files(&[
                    (
                        "test.ts",
                        "import Dep = require(\"./dep\")\nexport import Hidden = Dep.Hidden\nexport import Shown = Dep.Shown\n",
                    ),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Hidden { value: E }\nexport interface Shown { value: E }\n",
                        ]),
                    ),
                ]),
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            // A whole-module `require` alias is public; its tagged members are
            // filtered by the namespace walk, so the safe member does not warn.
            case(
                "whole module require filters internal member",
                files(&[
                    ("test.ts", "import Dep = require(\"./dep\")\nexport import Api = Dep\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Hidden { value: E }\nexport interface Safe { value: string }\n",
                        ]),
                    ),
                ]),
                &[],
                &["`Api` exposes `E`"],
            ),
            // A whole-module `require` alias still reports its public members.
            case(
                "whole module require checks public member",
                files(&[
                    ("test.ts", "import Dep = require(\"./dep\")\nexport import Api = Dep\n"),
                    (
                        "dep.ts",
                        &cat(&[
                            &experimental_e,
                            "/** @internal */\nexport interface Hidden { value: E }\nexport interface Shown { value: E }\n",
                        ]),
                    ),
                ]),
                &["`Api` exposes `E`"],
                &["`Api` exposes `Shown`"],
            ),
            // A public target keeps warning through the alias, exactly like a
            // named import alias.
            case(
                "public import equals target stays checked",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &namespace_impl,
                        "export import Shown = Impl.Shown\nexport { Shown as NormalAlias }\n",
                    ]),
                )]),
                &["`Shown` exposes `E`", "`NormalAlias` exposes `E`"],
                &[],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_test.go:1143 TestApiStabilityLeakImportEqualsIsMetadataOnly 3830d9d222930cb9
// TestApiStabilityLeakImportEqualsIsMetadataOnly pins the metadata
// boundary of the import-equals path: deciding that the alias forwards an
// `@internal` target resolves no type, so it adds no checker instantiation,
// produces no global diagnostic, and a repeated run is identical. The public
// peer is still analyzed, so the metadata decisions cannot poison it.
#[test]
fn test_api_stability_leak_import_equals_is_metadata_only() {
    run(|t| {
        t.parallel();

        let files = files(&[(
            "test.ts",
            r"/** @stability experimental */
interface E { value: string }
declare const e: E
namespace Impl {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: E }
}
export import Hidden = Impl.Hidden
export import Shown = Impl.Shown
",
        )]);

        let mut compiled = compile_api_stability_files(t, &files);
        compiled.with_rule_ctx(|rule_ctx| {
            let source_file = rule_ctx.source_file;
            if get_symbol_of_declaration(rule_ctx.checker(), source_file.as_node()).is_none() {
                t.fatal("missing module symbol");
            }

            let before = rule_ctx.checker().total_instantiation_count;
            let first = run_api_stability_messages(t, rule_ctx);
            let delta = rule_ctx.checker().total_instantiation_count - before;
            let second = run_api_stability_messages(t, rule_ctx);
            let delta2 = rule_ctx.checker().total_instantiation_count - before - delta;
            let globals = rule_ctx.checker().get_global_diagnostics();

            if delta != 0 {
                t.error(format!("import-equals filtering instantiated {delta} types, want 0"));
            }
            if delta2 != 0 {
                t.error(format!("repeated run instantiated {delta2} types, want 0"));
            }
            if !globals.is_empty() {
                let globals: Vec<String> =
                    globals.iter().map(|diagnostic| diagnostic.localize()).collect();
                t.error(format!(
                    "import-equals filtering produced global diagnostics: {globals:?}"
                ));
            }
            if first != second {
                t.error(format!("repeated runs disagree:\nfirst={first:?}\nsecond={second:?}"));
            }
            if contains_api_stability_message(t, &first, "`Hidden` exposes") {
                t.error(format!("tagged export was checked: {first:?}"));
            }
            if !contains_api_stability_message(t, &first, "`Shown` exposes `E`") {
                t.error(format!("public peer lost its diagnostic: {first:?}"));
            }
        });
    });
}

// go test: internal/rules/api_stability_leak_test.go:1199 TestApiStabilityLeakSurfaceRootTagSplit eb8c303419ab2741
// TestApiStabilityLeakSurfaceRootTagSplit pins the rule-facing
// side of the declared-versus-surface split: an export's own declared
// tag governs its ceiling and is never reported as its own dependency, while
// the export's children keep being reported even when their root is tagged.
#[test]
fn test_api_stability_leak_surface_root_tag_split() {
    run(|t| {
        t.parallel();

        let tests = vec![
            case(
                "tagged function root reports its children but not itself",
                single_api_stability_file(
                    "/** @stability unstable */\nexport function tagged(x: string): E",
                ),
                &["`tagged` exposes `E`"],
                &["`tagged` exposes `tagged`"],
            ),
            case(
                "tagged function root without a leak reports nothing",
                single_api_stability_file(
                    "/** @stability unstable */\nexport function clean(x: string): string",
                ),
                &[],
                &["exposes"],
            ),
            case(
                "tagged interface root reports its children but not itself",
                single_api_stability_file(
                    "/** @stability unstable */\nexport interface Root { member: E }",
                ),
                &["`Root` exposes `E`"],
                &["`Root` exposes `Root`"],
            ),
            case(
                "tagged alias root reports its children but not itself",
                single_api_stability_file(
                    "/** @stability unstable */\nexport type Root = { member: E }",
                ),
                &["`Root` exposes `E`"],
                &["`Root` exposes `Root`"],
            ),
            case(
                "tagged generic root keeps its constraint checked",
                single_api_stability_file(
                    "/** @stability unstable */\nexport declare function tagged<T extends E>(): T",
                ),
                &["`tagged` exposes `E`"],
                &["`tagged` exposes `tagged`"],
            ),
            case(
                "the same tagged type as root and as child",
                single_api_stability_file(
                    r"/** @stability experimental */
export interface Shared { value: string }
export interface Holder { shared: Shared }
",
                ),
                &["`Holder` exposes `Shared`"],
                &["`Shared` exposes `Shared`"],
            ),
            case(
                "an explicit stable child boundary keeps generic arguments",
                single_api_stability_file(
                    r"/** @stability stable */
export interface StableBox<T> { value: T }
export declare const boxed: StableBox<E>
",
                ),
                &["`boxed` exposes `E`"],
                &[],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_test.go:1277 TestApiStabilityLeakSurfaceChildBoundaryDifferentials fb5a46452546f6c0
// TestApiStabilityLeakSurfaceChildBoundaryDifferentials pins the
// rule-facing explicit child boundary: a child with an explicit declared tag
// contributes its declared level and its internals are not reported through the
// parent, while an untagged child keeps composing. Every exported tagged
// component is still audited as its own root, generic arguments of a tagged
// child stay exposed, and a same-named own override is never suppressed by a
// tagged base.
#[test]
fn test_api_stability_leak_surface_child_boundary_differentials() {
    run(|t| {
        t.parallel();

        let tests = vec![
            case(
                "heritage: tagged stable base is a boundary",
                single_api_stability_file(
                    r"/** @stability stable */
export interface TaggedBase { secret: E }
export interface Leaky extends TaggedBase {}
",
                ),
                &["`TaggedBase` exposes `E`"],
                &["`Leaky` exposes `E`", "`Leaky` exposes `TaggedBase`"],
            ),
            case(
                "heritage: tagged unstable base contributes its level only",
                single_api_stability_file(
                    r"/** @stability unstable */
export interface TaggedBase { secret: E }
export interface Leaky extends TaggedBase {}
",
                ),
                &["`TaggedBase` exposes `E`", "`Leaky` exposes `TaggedBase`, an unstable API."],
                &["`Leaky` exposes `E`"],
            ),
            case(
                "heritage control: untagged base is expanded",
                single_api_stability_file(
                    r"interface PlainBase { secret: E }
export interface PlainChild extends PlainBase {}
",
                ),
                &["`PlainChild` exposes `E`"],
                &[],
            ),
            case(
                "signature: tagged stable callable member is a boundary",
                single_api_stability_file(
                    r"/** @stability stable */
export interface TaggedCallable { (): E }
export interface Holder { callable: TaggedCallable }
",
                ),
                &["`TaggedCallable` exposes `E`"],
                &["`Holder` exposes `E`"],
            ),
            case(
                "signature control: untagged callable member is inspected",
                single_api_stability_file(
                    r"interface PlainCallable { (): E }
export interface Holder { callable: PlainCallable }
",
                ),
                &["`Holder` exposes `E`"],
                &[],
            ),
            case(
                "member: tagged member is a boundary for its anonymous type",
                single_api_stability_file(
                    r"export interface Holder {
  /** @stability stable */
  nested: { inner: E }
}
",
                ),
                &[],
                &["`Holder` exposes"],
            ),
            case(
                "member control: untagged member keeps the anonymous walk",
                single_api_stability_file(
                    r"export interface Holder {
  nested: { inner: E }
}
",
                ),
                &["`Holder` exposes `E`"],
                &[],
            ),
            case(
                "signature: tagged call signature is its own boundary",
                single_api_stability_file(
                    r"export interface Holder {
  /** @stability stable */
  (): E
}
",
                ),
                &[],
                &["`Holder` exposes"],
            ),
            case(
                "signature control: untagged call signature is walked",
                single_api_stability_file(
                    r"export interface Holder {
  (): E
}
",
                ),
                &["`Holder` exposes `E`"],
                &[],
            ),
            case(
                "index: tagged index signature is its own boundary",
                single_api_stability_file(
                    r"export interface Holder {
  /** @stability stable */
  [key: string]: E
}
",
                ),
                &[],
                &["`Holder` exposes"],
            ),
            case(
                "index control: untagged index signature is walked",
                single_api_stability_file(
                    r"export interface Holder {
  [key: string]: E
}
",
                ),
                &["`Holder` exposes `E`"],
                &[],
            ),
            case(
                "generic argument: tagged child argument stays exposed",
                single_api_stability_file(
                    r"/** @stability stable */
export interface StableBox<T> { value: T }
export interface BoxHolder { box: StableBox<E> }
",
                ),
                &["`BoxHolder` exposes `E`"],
                &[],
            ),
            case(
                "same component: tagged child boundary and root audit",
                single_api_stability_file(
                    r"/** @stability unstable */
export interface TaggedChild { secret: E }
export interface Holder { child: TaggedChild }
",
                ),
                &["`TaggedChild` exposes `E`", "`Holder` exposes `TaggedChild`, an unstable API."],
                &["`Holder` exposes `E`"],
            ),
            case(
                "deep heritage: untagged intermediate keeps the tagged argument",
                single_api_stability_file(
                    r"/** @stability stable */
export interface TaggedChain<T> { value: T }
export interface Mid<U> extends TaggedChain<U> {}
export interface Deep extends Mid<E> {}
",
                ),
                &["`Deep` exposes `E`"],
                &[],
            ),
            case(
                "heritage override: own member is not suppressed by a tagged base",
                single_api_stability_file(
                    r"/** @stability stable */
export interface TaggedBase { secret: E }
export interface Overriding extends TaggedBase { secret: E }
",
                ),
                &["`Overriding` exposes `E`"],
                &["`Overriding` exposes `TaggedBase`"],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_test.go:1437 TestApiStabilityLeakSurfaceFlattenedInheritanceDifferentials 9e52d1499dd6f9d5
// TestApiStabilityLeakSurfaceFlattenedInheritanceDifferentials
// pins the rule-facing side of the fix2 correction: call, construct and index
// signatures the compiler flattens into a derived structured type keep their
// declaring base provenance, so an explicitly tagged base bounds those
// internals for derived declarations and exported values. Each tagged base is
// still audited as its own root, own constructor/call/index overrides stay
// reported, independently represented generic arguments stay exposed and
// untagged controls keep composing. Every case runs warm and cold; the cold
// harness additionally proves the rule never changes compiler diagnostics.
#[test]
fn test_api_stability_leak_surface_flattened_inheritance_differentials() {
    run(|t| {
        t.parallel();

        let tests = vec![
            case(
                "inherited call signature from tagged base",
                single_api_stability_file(
                    r"/** @stability experimental */
interface FixCallE { value: string }
/** @stability unstable */
export interface FixTaggedCallableBase { (): FixCallE }
export interface FixDerivedCallableGeneric<T> extends FixTaggedCallableBase { own: T }
export declare const fixCallRef: FixDerivedCallableGeneric<string>
interface FixPlainCallableBase { (): FixCallE }
export interface FixPlainDerivedCallableGeneric<T> extends FixPlainCallableBase { own: T }
export declare const fixPlainCallRef: FixPlainDerivedCallableGeneric<string>
",
                ),
                &[
                    "`FixTaggedCallableBase` exposes `FixCallE`",
                    "`fixPlainCallRef` exposes `FixCallE`",
                    "`FixDerivedCallableGeneric` exposes `FixTaggedCallableBase`, an unstable API.",
                    "`fixCallRef` exposes `FixTaggedCallableBase`, an unstable API.",
                ],
                &[
                    "`fixCallRef` exposes `FixCallE`",
                    "`FixDerivedCallableGeneric` exposes `FixCallE`",
                ],
            ),
            case(
                "inherited index signature from tagged base",
                single_api_stability_file(
                    r"/** @stability experimental */
interface FixIdxE { value: string }
/** @stability unstable */
export interface FixTaggedIndexBase { [key: string]: FixIdxE }
export interface FixDerivedIndexGeneric<T> extends FixTaggedIndexBase { own: T }
export declare const fixIndexRef: FixDerivedIndexGeneric<string>
interface FixPlainIndexBase { [key: string]: FixIdxE }
export interface FixPlainDerivedIndexGeneric<T> extends FixPlainIndexBase { own: T }
export declare const fixPlainIndexRef: FixPlainDerivedIndexGeneric<string>
",
                ),
                &["`FixTaggedIndexBase` exposes `FixIdxE`", "`fixPlainIndexRef` exposes `FixIdxE`"],
                &["`fixIndexRef` exposes `FixIdxE`", "`FixDerivedIndexGeneric` exposes `FixIdxE`"],
            ),
            case(
                "inherited call construct and index signatures from one tagged base",
                single_api_stability_file(
                    r"/** @stability experimental */
interface FixComboCallE { value: string }
/** @stability experimental */
interface FixComboCtorE { value: string }
/** @stability experimental */
interface FixComboIdxE { value: string }
/** @stability unstable */
export interface FixComboBase {
  (): FixComboCallE
  new (): FixComboCtorE
  [key: string]: FixComboIdxE
}
export interface FixComboDerived<T> extends FixComboBase { own: T }
export declare const fixComboRef: FixComboDerived<string>
",
                ),
                &[
                    "`FixComboBase` exposes `FixComboCallE`",
                    "`FixComboBase` exposes `FixComboCtorE`",
                    "`FixComboBase` exposes `FixComboIdxE`",
                ],
                &[
                    "`fixComboRef` exposes `FixComboCallE`",
                    "`fixComboRef` exposes `FixComboCtorE`",
                    "`fixComboRef` exposes `FixComboIdxE`",
                    "`FixComboDerived` exposes `FixComboCallE`",
                    "`FixComboDerived` exposes `FixComboCtorE`",
                    "`FixComboDerived` exposes `FixComboIdxE`",
                ],
            ),
            case(
                "inherited constructor from tagged class",
                single_api_stability_file(
                    r"/** @stability experimental */
interface FixCtorE { value: string }
/** @stability experimental */
interface FixStaticE { value: string }
/** @stability experimental */
interface FixMethodE { value: string }
/** @stability unstable */
export class FixTaggedBaseClass {
  constructor(x: FixCtorE)
  static s: FixStaticE
  m(): FixMethodE
}
export class FixDerivedClass extends FixTaggedBaseClass {}
",
                ),
                &[
                    "`FixTaggedBaseClass` exposes `FixCtorE`",
                    "`FixTaggedBaseClass` exposes `FixStaticE`",
                    "`FixTaggedBaseClass` exposes `FixMethodE`",
                ],
                &[
                    "`FixDerivedClass` exposes `FixCtorE`",
                    "`FixDerivedClass` exposes `FixStaticE`",
                    "`FixDerivedClass` exposes `FixMethodE`",
                ],
            ),
            case(
                "own derived constructor overrides the tagged base constructor",
                single_api_stability_file(
                    r"/** @stability experimental */
interface FixOwnCtorE { value: string }
/** @stability unstable */
export class FixOwnCtorBase { constructor(x: FixOwnCtorE) }
export class FixOwnCtorDerived extends FixOwnCtorBase {
  constructor(y: E) { super(y as unknown as FixOwnCtorE) }
}
",
                ),
                &["`FixOwnCtorDerived` exposes `E`"],
                &["`FixOwnCtorDerived` exposes `FixOwnCtorE`"],
            ),
            case(
                "untagged intermediate chain ending in a tagged callable ancestor",
                single_api_stability_file(
                    r"/** @stability experimental */
interface FixDeepE { value: string }
/** @stability unstable */
export interface FixTaggedDeepCallable { (): FixDeepE }
export interface FixMidCallable<T> extends FixTaggedDeepCallable { mid: T }
export interface FixDeepCallable<T> extends FixMidCallable<T> {}
export declare const fixDeepCallRef: FixDeepCallable<string>
",
                ),
                &["`FixTaggedDeepCallable` exposes `FixDeepE`"],
                &["`fixDeepCallRef` exposes `FixDeepE`", "`FixDeepCallable` exposes `FixDeepE`"],
            ),
            case(
                "explicit stable callable base stays quiet",
                single_api_stability_file(
                    r"/** @stability experimental */
interface FixStableE { value: string }
/** @stability stable */
export interface FixStableCallableBase { (): FixStableE }
export interface FixDerivedStableCallable extends FixStableCallableBase {}
export declare const fixStableRef: FixDerivedStableCallable
",
                ),
                &[],
                &["`FixDerivedStableCallable` exposes", "`fixStableRef` exposes"],
            ),
            case(
                "own call and index signatures override tagged base signatures",
                single_api_stability_file(
                    r"/** @stability experimental */
interface FixOverrideCallE { value: string }
/** @stability experimental */
interface FixOverrideIdxE { value: string }
/** @stability unstable */
export interface FixOverrideBase { (): FixOverrideCallE; [key: string]: FixOverrideIdxE }
export interface FixOverrideDerived extends FixOverrideBase {
  (): E
  [key: string]: E
}
",
                ),
                &["`FixOverrideDerived` exposes `E`"],
                &[
                    "`FixOverrideDerived` exposes `FixOverrideCallE`",
                    "`FixOverrideDerived` exposes `FixOverrideIdxE`",
                ],
            ),
            case(
                "tagged ancestor generic argument stays exposed",
                single_api_stability_file(
                    r"/** @stability experimental */
interface FixBoxHiddenE { value: string }
/** @stability experimental */
interface FixBoxArgE { value: string }
/** @stability unstable */
export interface FixTaggedCallableBox<T> { (): FixBoxHiddenE; value: T }
export interface FixDerivedCallableBox<T> extends FixTaggedCallableBox<T> {}
export declare const fixCallableBoxRef: FixDerivedCallableBox<FixBoxArgE>
",
                ),
                &[
                    "`fixCallableBoxRef` exposes `FixBoxArgE`",
                    "`FixTaggedCallableBox` exposes `FixBoxHiddenE`",
                ],
                &["`fixCallableBoxRef` exposes `FixBoxHiddenE`"],
            ),
            case(
                "blocked required argument of a tagged ancestor stays incomplete and keeps internals bounded",
                single_api_stability_file(
                    r"/** @stability experimental */
interface FixBlockedHiddenE { value: string }
/** @stability unstable */
export interface FixBlockedBox<T> { value: T; hidden: FixBlockedHiddenE }
export interface FixBlockedDerived<T, K extends keyof T> extends FixBlockedBox<T[K]> {}
",
                ),
                &[],
                &["`FixBlockedDerived` exposes `FixBlockedHiddenE`"],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_test.go:1654 TestApiStabilityLeakSurfaceSharedSnapshotReuse e273462f0f71f20d
// TestApiStabilityLeakSurfaceSharedSnapshotReuse pins the
// rule-facing side of the shared caches: running the rule twice over
// one checker must report exactly the same diagnostics, because the second run
// composes the complete type and signature snapshots the first run published.
// A snapshot that belongs to a tagged component is only ever reached as that
// component's own root audit; as a child it contributes its declared level and
// never leaks the snapshot internals, in either export order.
#[test]
fn test_api_stability_leak_surface_shared_snapshot_reuse() {
    run(|t| {
        t.parallel();

        let files = single_api_stability_file(
            r"/** @stability unstable */
export interface ATagged { leak: E }
export interface ZHolder { child: ATagged }
/** @stability unstable */
export interface ZTagged { leak: E }
export interface AHolder { child: ZTagged }
/** @stability stable */
export interface StableBox<T> { value: T }
export interface BoxHolder { box: StableBox<E> }
export declare function makeTagged(): ZTagged
",
        );
        let mut compiled = compile_api_stability_files(t, &files);
        compiled.with_rule_ctx(|rule_ctx| {
            let first = run_api_stability_messages(t, rule_ctx);
            let second = run_api_stability_messages(t, rule_ctx);
            let mut sorted_first = first.clone();
            let mut sorted_second = second;
            sorted_first.sort();
            sorted_second.sort();
            if sorted_first != sorted_second {
                t.fatal(format!(
                    "second run over the shared snapshots disagrees:\nfirst={sorted_first:?}\nsecond={sorted_second:?}"
                ));
            }

            for want in [
                "`ATagged` exposes `E`",
                "`ZTagged` exposes `E`",
                "`ZHolder` exposes `ATagged`, an unstable API.",
                "`AHolder` exposes `ZTagged`, an unstable API.",
                "`BoxHolder` exposes `E`",
                "`makeTagged` exposes `ZTagged`, an unstable API.",
            ] {
                if !contains_api_stability_message(t, &first, want) {
                    t.error(format!("missing diagnostic {want:?} in {first:?}"));
                }
            }
            for not_want in [
                "`ZHolder` exposes `E`",
                "`AHolder` exposes `E`",
                "`makeTagged` exposes `E`",
                "`ATagged` exposes `ATagged`",
                "`ZTagged` exposes `ZTagged`",
            ] {
                if contains_api_stability_message(t, &first, not_want) {
                    t.error(format!("unexpected diagnostic {not_want:?} in {first:?}"));
                }
            }
        });
    });
}
