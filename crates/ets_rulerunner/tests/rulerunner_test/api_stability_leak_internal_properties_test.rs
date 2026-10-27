// Port of internal/rulerunner/api_stability_leak_internal_properties_test.go.

use ets_go::testing::{on_checker_stack, run};

use crate::api_stability_leak_test::{
    api_stability_lifecycle_contains, api_stability_lifecycle_diagnostics,
};

struct InternalPropertiesCase {
    name: &'static str,
    source: String,
    want: bool,
}

// go test: internal/rulerunner/api_stability_leak_internal_properties_test.go:8 TestApiStabilityLeakInternalProperties 8379452175d9567c
#[test]
fn test_api_stability_leak_internal_properties() {
    on_checker_stack(|| {
        run(|t| {
            t.parallel();
            let prefix = "/** @stability experimental */\ninterface E { value: string }\n";
            let tests = [
                InternalPropertiesCase {
                    name: "interface property",
                    source: prefix.to_string()
                        + r"export interface Api {
/** @internal */ hidden?: E
visible?: string
}",
                    want: false,
                },
                InternalPropertiesCase {
                    name: "tagged internal property",
                    source: prefix.to_string()
                        + r"export interface Api {
/** @internal
 * @stability experimental */ hidden: E
}",
                    want: false,
                },
                InternalPropertiesCase {
                    name: "public peer stays checked",
                    source: prefix.to_string()
                        + r"export interface Api {
/** @internal */ hidden?: E
visible?: E
}",
                    want: true,
                },
                InternalPropertiesCase {
                    name: "internal referenced type stays checked",
                    source: "/** @internal\n * @stability experimental */\ninterface E { value: string }\nexport interface Api { visible: E }".to_string(),
                    want: true,
                },
                InternalPropertiesCase {
                    name: "class instance and static properties",
                    source: prefix.to_string()
                        + r"export class Api {
/** @internal */ hidden!: E
/** @internal */ static hidden?: E
}",
                    want: false,
                },
                InternalPropertiesCase {
                    name: "internal method and accessor",
                    source: prefix.to_string()
                        + r"export class Api {
/** @internal */ hidden(): E { return null as unknown as E }
/** @internal */ get value(): E { return null as unknown as E }
}",
                    want: false,
                },
                InternalPropertiesCase {
                    name: "inherited internal property",
                    source: prefix.to_string()
                        + r"interface Base {
/** @internal */ hidden?: E
}
export interface Api extends Base {}",
                    want: false,
                },
                InternalPropertiesCase {
                    name: "generic internal property",
                    source: prefix.to_string()
                        + r"interface Generic<T> {
/** @internal */ hidden?: T
}
export declare const Api: Generic<E>",
                    // A represented generic argument is itself part of the public API,
                    // even when a property using that argument is internal.
                    want: true,
                },
                InternalPropertiesCase {
                    name: "internal anonymous property with erased alias",
                    source: r"/** @stability experimental */
type E = string
export type Api = {
/** @internal */ hidden?: E
visible?: string
}"
                    .to_string(),
                    want: false,
                },
                InternalPropertiesCase {
                    name: "internal method with erased alias",
                    source: r"/** @stability experimental */
type E = string
export type Api = {
/** @internal */ hidden(): E
}"
                    .to_string(),
                    want: false,
                },
                InternalPropertiesCase {
                    name: "merged public property remains checked",
                    source: prefix.to_string()
                        + r"export interface Api {
/** @internal */ hidden?: E
}
export interface Api { hidden?: E }",
                    want: true,
                },
                InternalPropertiesCase {
                    name: "merged internal properties",
                    source: prefix.to_string()
                        + r"export interface Api {
/** @internal */ hidden?: E
}
export interface Api {
/** @internal */ hidden?: E
}",
                    want: false,
                },
            ];
            for test in &tests {
                for whole_program in [false, true] {
                    let mode = if whole_program { "program" } else { "file" };
                    t.run(&format!("{}/{mode}", test.name), |t| {
                        t.parallel();
                        let files = [("test.ts", test.source.as_str())];
                        let messages =
                            api_stability_lifecycle_diagnostics(t, &files, true, whole_program);
                        let got = api_stability_lifecycle_contains(&messages, "`Api` exposes");
                        if got != test.want {
                            t.error(format!("Api leak={got}, want {}: {messages:?}", test.want));
                        }
                        let control =
                            api_stability_lifecycle_diagnostics(t, &files, false, whole_program);
                        for message in &messages {
                            if !message.contains("effect(apiStabilityLeak)")
                                && !api_stability_lifecycle_contains(&control, message)
                            {
                                t.error(format!("rule introduced compiler diagnostic {message:?}"));
                            }
                        }
                    });
                }
            }
        });
    });
}

// go test: internal/rulerunner/api_stability_leak_internal_properties_test.go:136 TestApiStabilityLeakSchemaAnnotationAugmentation 23a276b1b1369ecd
// Mirrors Effect's HttpApiSchema augmentation of Schema.Annotations.Augment:
// the public status annotation remains visible, while the internal encoding
// and headers payloads must not lower the schema annotation surface's stability.
#[test]
fn test_api_stability_leak_schema_annotation_augmentation() {
    on_checker_stack(|| {
        run(|t| {
            t.parallel();
            for public_encoding in [false, true] {
                for whole_program in [false, true] {
                    let mut name = if public_encoding { "public" } else { "internal" }.to_string();
                    if whole_program {
                        name += "/program";
                    } else {
                        name += "/file";
                    }
                    t.run(&name, |t| {
                        t.parallel();
                        let internal_tag =
                            if public_encoding { "Encoding payload." } else { "@internal" };
                        let test_source = r#"import { Annotations } from "./schema"
/** @stability experimental */
interface Encoding { readonly kind: string }
/** @stability unstable */
interface WithHeadersAnnotation { readonly headers: object }
declare module "./schema" {
namespace Annotations {
interface Augment {
readonly httpApiStatus?: number | undefined
/** "#
                            .to_string()
                            + internal_tag
                            + r#" */
readonly "~httpApiEncoding"?: Encoding | undefined
/** @internal */
readonly "~httpApiWithHeaders"?: WithHeadersAnnotation | undefined
}
}
}
export interface Api extends Annotations.Augment {}
export declare const api: Api
export type Literal = {
/** @internal */ encoding?: Encoding
readonly httpApiStatus?: number
}
"#;
                        let files = [
                            (
                                "schema.ts",
                                r"export namespace Annotations {
export interface Augment { readonly title?: string }
}
export interface Expanded extends Annotations.Augment {}
",
                            ),
                            ("test.ts", test_source.as_str()),
                        ];
                        let messages =
                            api_stability_lifecycle_diagnostics(t, &files, true, whole_program);
                        for export in ["Api", "api", "Expanded", "Annotations"] {
                            if (export == "Annotations" || export == "Expanded") && !whole_program
                            {
                                continue;
                            }
                            let got = api_stability_lifecycle_contains(
                                &messages,
                                &format!("`{export}` exposes `Encoding`"),
                            );
                            if got != public_encoding {
                                t.error(format!(
                                    "{export} Encoding leak={got}, want {public_encoding}: {messages:?}"
                                ));
                            }
                        }
                        for message in &messages {
                            if message.contains("exposes `WithHeadersAnnotation`")
                                || message.contains("`Literal` exposes")
                            {
                                t.error(format!("internal annotation payload leaked: {message}"));
                            }
                        }
                    });
                }
            }
        });
    });
}
