// Port of internal/rulerunner/api_stability_leak_inheritance_test.go.

use ets_go::testing::{on_checker_stack, run};

use crate::api_stability_leak_test::{
    api_stability_lifecycle_contains, api_stability_lifecycle_diagnostics,
};

struct OptionalTaggedInheritanceCase {
    name: &'static str,
    members: &'static str,
    extra: &'static str,
    want: bool,
}

const fn optional_case(
    name: &'static str,
    members: &'static str,
    extra: &'static str,
    want: bool,
) -> OptionalTaggedInheritanceCase {
    OptionalTaggedInheritanceCase { name, members, extra, want }
}

// go test: internal/rulerunner/api_stability_leak_inheritance_test.go:8 TestApiStabilityLeakOptionalTaggedInheritance 4648a503e69d8746
#[test]
fn test_api_stability_leak_optional_tagged_inheritance() {
    on_checker_stack(|| {
        run(|t| {
            t.parallel();
            let tests = [
                optional_case("empty base", "", "", false),
                optional_case(
                    "optional unstable property",
                    "/** @stability unstable */ test?: string",
                    "",
                    false,
                ),
                optional_case(
                    "optional experimental property",
                    "/** @stability experimental */ test?: string",
                    "",
                    false,
                ),
                optional_case(
                    "optional stable property",
                    "/** @stability stable */ test?: string",
                    "",
                    false,
                ),
                optional_case(
                    "optional tagged method",
                    "/** @stability unstable */ test?(): string",
                    "",
                    false,
                ),
                optional_case(
                    "optional internal property",
                    "/** @internal */ test?: string",
                    "",
                    false,
                ),
                optional_case(
                    "required internal property",
                    "/** @internal */ test: string",
                    "",
                    false,
                ),
                optional_case(
                    "public optional and internal required",
                    "/** @stability unstable */ test?: string;\n/** @internal */ hidden: string",
                    "",
                    false,
                ),
                optional_case(
                    "required tagged property",
                    "/** @stability unstable */ test: string",
                    "",
                    true,
                ),
                optional_case(
                    "required tagged method",
                    "/** @stability unstable */ test(): string",
                    "",
                    true,
                ),
                optional_case("optional untagged property", "test?: string", "", true),
                optional_case(
                    "mixed members",
                    "/** @stability unstable */ test?: string; other?: string",
                    "",
                    true,
                ),
                optional_case(
                    "invalid stability tag",
                    "/** @stability unknown */ test?: string",
                    "",
                    true,
                ),
                optional_case(
                    "index signature",
                    "/** @stability unstable */ [key: string]: string",
                    "",
                    true,
                ),
                optional_case("call signature", "/** @stability unstable */ (): string", "", true),
                optional_case(
                    "construct signature",
                    "/** @stability unstable */ new (): object",
                    "",
                    true,
                ),
                optional_case(
                    "merged required member",
                    "/** @stability unstable */ test?: string",
                    "interface Reportable { other: string }",
                    true,
                ),
            ];
            for test in &tests {
                for whole_program in [false, true] {
                    let mode = if whole_program { "program" } else { "file" };
                    t.run(&format!("{}/{mode}", test.name), |t| {
                        t.parallel();
                        let source = "/** @stability unstable */\ninterface Reportable {\n".to_string()
                            + test.members
                            + "\n}\n"
                            + test.extra
                            + r"
declare global { interface Error extends Reportable {} }
export function x(e: Error) {}
export interface Expanded extends Error {}
export declare const expanded: Expanded
export function direct(e: Reportable) {}
export interface Ordinary extends Reportable {}
";
                        let files = [("test.ts", source.as_str())];
                        let messages =
                            api_stability_lifecycle_diagnostics(t, &files, true, whole_program);
                        let control =
                            api_stability_lifecycle_diagnostics(t, &files, false, whole_program);
                        for name in ["x", "Expanded", "expanded", "Ordinary"] {
                            let got = api_stability_lifecycle_contains(
                                &messages,
                                &format!("`{name}` exposes `Reportable`"),
                            );
                            if got != test.want {
                                t.error(format!(
                                    "{name} leak={got}, want {}: {messages:?}",
                                    test.want
                                ));
                            }
                        }
                        #[expect(clippy::single_element_loop, reason = "Go loops over a one-element list")]
                        for name in ["direct"] {
                            if !api_stability_lifecycle_contains(
                                &messages,
                                &format!("`{name}` exposes `Reportable`"),
                            ) {
                                t.error(format!(
                                    "direct/ordinary exposure of {name} must still report: {messages:?}"
                                ));
                            }
                        }
                        for message in &messages {
                            if !message.contains("effect(apiStabilityLeak)")
                                && !api_stability_lifecycle_contains(&control, message)
                            {
                                t.error(format!("rule introduced compiler diagnostic {message:?}"));
                            }
                        }
                        if !test.want {
                            for name in ["x", "Expanded", "expanded", "Ordinary"] {
                                if api_stability_lifecycle_contains(
                                    &messages,
                                    &format!("`{name}` exposes"),
                                ) {
                                    t.error(format!(
                                        "optional augmentation must not leak member tags for {name}: {messages:?}"
                                    ));
                                }
                            }
                        }
                    });
                }
            }
        });
    });
}

struct InheritancePathCase {
    name: &'static str,
    source: String,
    dep: String,
    want: bool,
}

// go test: internal/rulerunner/api_stability_leak_inheritance_test.go:80 TestApiStabilityLeakInheritancePaths 6418f38ec8bc9741
#[test]
fn test_api_stability_leak_inheritance_paths() {
    on_checker_stack(|| {
        run(|t| {
            t.parallel();
            let prefix = r"/** @stability unstable */
interface Reportable {
/** @stability unstable */ test?: string }
";
            let exported_prefix =
                prefix.replacen("interface Reportable", "export interface Reportable", 1);
            let tests = [
                InheritancePathCase {
                    name: "module augmentation",
                    dep: "export interface Target {}".to_string(),
                    source: prefix.to_string()
                        + r#"import { Target } from "./dep"
declare module "./dep" { interface Target extends Reportable {} }
export function x(e: Target) {}
export interface Expanded extends Target {}"#,
                    want: false,
                },
                InheritancePathCase {
                    name: "module augmentation of class",
                    dep: "export class Target {}".to_string(),
                    source: prefix.to_string()
                        + r#"import { Target } from "./dep"
declare module "./dep" { interface Target extends Reportable {} }
export function x(e: Target) {}
export interface Expanded extends Target {}"#,
                    want: false,
                },
                InheritancePathCase {
                    name: "imported augmentation base",
                    dep: exported_prefix.clone(),
                    source: r#"import { Reportable as Imported } from "./dep"
declare global { interface Error extends Imported {} }
export function x(e: Error) {}
export interface Expanded extends Error {}"#
                        .to_string(),
                    want: false,
                },
                InheritancePathCase {
                    name: "ordinary and augmentation edges",
                    dep: exported_prefix + "\nexport interface Target extends Reportable {}",
                    source: r#"import { Target, Reportable } from "./dep"
declare module "./dep" { interface Target extends Reportable {} }
export function x(e: Target) {}
export interface Expanded extends Target {}"#
                        .to_string(),
                    want: false,
                },
                InheritancePathCase {
                    name: "ordinary route beside ignored route",
                    dep: String::new(),
                    source: prefix.to_string()
                        + r"declare global { interface Error extends Reportable {} }
interface Mixed extends Error, Reportable {}
export function x(e: Mixed) {}
export interface Expanded extends Mixed {}",
                    want: false,
                },
                InheritancePathCase {
                    name: "inherited optional tagged members",
                    dep: String::new(),
                    source: r"interface Parent {
/** @stability experimental */ parent?: string }
/** @stability unstable */
interface Reportable extends Parent {
/** @stability stable */ test?: string }
declare global { interface Error extends Reportable {} }
export function x(e: Error) {}
export interface Expanded extends Error {}"
                        .to_string(),
                    want: false,
                },
                InheritancePathCase {
                    name: "inherited required member",
                    dep: String::new(),
                    source: r"interface Parent { parent: string }
/** @stability unstable */
interface Reportable extends Parent {
/** @stability unstable */ test?: string }
declare global { interface Error extends Reportable {} }
export function x(e: Error) {}
export interface Expanded extends Error {}"
                        .to_string(),
                    want: true,
                },
                InheritancePathCase {
                    name: "class base with optional tagged members",
                    dep: String::new(),
                    source: r"/** @stability unstable */
class Reportable {
/** @stability unstable */ test?: string
}
declare global { interface Error extends Reportable {} }
export function x(e: Error) {}
export interface Expanded extends Error {}"
                        .to_string(),
                    want: false,
                },
                InheritancePathCase {
                    name: "class inheriting augmented Error",
                    dep: String::new(),
                    source: prefix.to_string()
                        + r"declare global { interface Error extends Reportable {} }
export class Expanded extends Error {}
export function x(e: Expanded) {}",
                    want: false,
                },
                InheritancePathCase {
                    name: "generic augmentation arguments stay optional",
                    dep: String::new(),
                    source: r"/** @stability experimental */
interface E { value: string }
/** @stability unstable */
interface Reportable<T> {
/** @stability unstable */ test?: T
}
declare global { interface Error extends Reportable<E> {} }
export function x(e: Error) {}
export interface Expanded extends Error {}"
                        .to_string(),
                    want: false,
                },
                InheritancePathCase {
                    name: "untagged base with tagged optional members",
                    dep: String::new(),
                    source: r"interface Reportable {
/** @stability experimental */ test?: string
}
declare global { interface Error extends Reportable {} }
export function x(e: Error) {}
export interface Expanded extends Error {}"
                        .to_string(),
                    want: false,
                },
                InheritancePathCase {
                    name: "own override remains exposed",
                    dep: String::new(),
                    source: prefix.to_string()
                        + r"declare global { interface Error extends Reportable {} }
export interface Expanded extends Error {
/** @stability experimental */ test?: string }
export function x(e: Reportable) {}",
                    want: true,
                },
            ];
            for test in &tests {
                for whole_program in [false, true] {
                    let mode = if whole_program { "program" } else { "file" };
                    t.run(&format!("{}/{mode}", test.name), |t| {
                        t.parallel();
                        let mut files = vec![("test.ts", test.source.as_str())];
                        if !test.dep.is_empty() {
                            files.push(("dep.ts", test.dep.as_str()));
                        }
                        let messages =
                            api_stability_lifecycle_diagnostics(t, &files, true, whole_program);
                        for name in ["x", "Expanded"] {
                            let got = api_stability_lifecycle_contains(
                                &messages,
                                &format!("`{name}` exposes"),
                            );
                            if got != test.want {
                                t.error(format!(
                                    "{name} leak={got}, want {}: {messages:?}",
                                    test.want
                                ));
                            }
                        }
                    });
                }
            }
        });
    });
}
