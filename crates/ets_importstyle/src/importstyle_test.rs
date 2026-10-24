use ets_etscore::ResolvedEffectPluginOptions;
use ets_go::sprintf;

use crate::*;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

// go test: internal/importstyle/importstyle_test.go:9 TestPolicyStyle
#[test]
fn test_policy_style() {
    ets_go::testing::run(|t| {
        t.parallel();
        let options = ResolvedEffectPluginOptions {
            namespace_import_packages: strings(&["Effect", "shared"]),
            barrel_import_packages: strings(&["@effect/platform", "shared"]),
            ..Default::default()
        };
        let policy = new_policy(Some(&options));
        let tests: &[(&str, Style)] = &[
            ("effect", Style::Namespace),
            ("effect/Effect", Style::Namespace),
            ("EFFECT/Array", Style::Namespace),
            ("@effect/platform", Style::Barrel),
            ("@effect/platform/HttpClient", Style::Barrel),
            ("shared/Thing", Style::Namespace),
            ("lodash", Style::None),
            ("./effect", Style::None),
            ("", Style::None),
        ];
        for &(specifier, expected) in tests {
            let got = policy.style(specifier);
            if got != expected {
                t.error(sprintf!("Style(%q) = %d, want %d", specifier, got, expected));
            }
        }
    });
}

// go test: internal/importstyle/importstyle_test.go:36 TestPolicyIsEmpty
#[test]
fn test_policy_is_empty() {
    ets_go::testing::run(|t| {
        t.parallel();
        if !new_policy(None).is_empty() {
            t.error("nil options should produce an empty policy");
        }
        if !new_policy(Some(&ResolvedEffectPluginOptions::default())).is_empty() {
            t.error("no packages should produce an empty policy");
        }
        let options = ResolvedEffectPluginOptions {
            barrel_import_packages: strings(&["effect"]),
            ..Default::default()
        };
        if new_policy(Some(&options)).is_empty() {
            t.error("a barrel package should produce a non-empty policy");
        }
    });
}

// go test: internal/importstyle/importstyle_test.go:49 TestPackageName
#[test]
fn test_package_name() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: &[(&str, &str)] = &[
            ("effect", "effect"),
            ("effect/Effect", "effect"),
            ("@scope/pkg", "@scope/pkg"),
            ("@scope/pkg/sub", "@scope/pkg"),
            ("@scope/pkg/sub/deep", "@scope/pkg"),
            ("./local", ""),
            ("/absolute", ""),
            ("", ""),
            ("@scope", "@scope"),
        ];
        for &(input, expected) in tests {
            let got = package_name(input);
            if got != expected {
                t.error(sprintf!("PackageName(%q) = %q, want %q", input, got, expected));
            }
        }
    });
}

// go test: internal/importstyle/importstyle_test.go:72 TestNamespaceName
#[test]
fn test_namespace_name() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: &[(&str, &str)] = &[
            ("effect/Effect", "Effect"),
            ("effect", "effect"),
            ("@scope/pkg/Foo", "Foo"),
            ("@effect/platform/HttpClient", "HttpClient"),
            ("", ""),
            ("foo/", ""),
        ];
        for &(input, expected) in tests {
            let got = namespace_name(input);
            if got != expected {
                t.error(sprintf!("NamespaceName(%q) = %q, want %q", input, got, expected));
            }
        }
    });
}

// go test: internal/importstyle/importstyle_test.go:92 TestReexportSpecifier
#[test]
fn test_reexport_specifier() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: &[(&str, &str, &str)] = &[
            ("effect", "./Effect.ts", "effect/Effect"),
            ("effect", "./Function.js", "effect/Function"),
            ("effect/http", "./HttpClient.d.ts", "effect/http/HttpClient"),
            ("@effect/platform", "./HttpClient", "@effect/platform/HttpClient"),
            ("effect", "./internal/core.ts", ""),
            ("effect", "../Effect.ts", ""),
            ("effect", "./index.js", ""),
            ("effect", "other", ""),
            ("", "./Effect.ts", ""),
        ];
        for &(barrel, relative, expected) in tests {
            let got = reexport_specifier(barrel, relative);
            if got != expected {
                t.error(sprintf!(
                    "ReexportSpecifier(%q, %q) = %q, want %q",
                    barrel,
                    relative,
                    got,
                    expected
                ));
            }
        }
    });
}
