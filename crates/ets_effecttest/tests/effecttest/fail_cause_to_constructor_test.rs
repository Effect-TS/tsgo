// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/fail_cause_to_constructor_test.go:11 TestFailCauseToConstructorFixesCompile addaf9903bda56d2
fn test_fail_cause_to_constructor_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let code = tsdiag::EFFECT_0_EXPRESSES_THIS_FAILURE_DIRECTLY_INSTEAD_OF_WRAPPING_CAUSE_2_IN_EFFECT_1_EFFECT_FAIL_CAUSE_TO_CONSTRUCTOR.code();
    verify_fixes_compile(
        t,
        EffectVersion::V3,
        "failCauseToConstructor",
        code,
        |title| title.starts_with("Replace with Effect."),
        &[
            FixCompileCase { fixture: "failCauseToConstructor.ts", matches: 24, fixes: 21 },
            FixCompileCase {
                fixture: "failCauseToConstructor_namedImports.ts",
                matches: 3,
                fixes: 0,
            },
            FixCompileCase {
                fixture: "failCauseToConstructor_aliasedEffect.ts",
                matches: 2,
                fixes: 2,
            },
            FixCompileCase {
                fixture: "failCauseToConstructor_lazyObjectLiteral.ts",
                matches: 1,
                fixes: 1,
            },
        ],
    );
    verify_fixes_compile(
        t,
        EffectVersion::V4,
        "failCauseToConstructor",
        code,
        |title| title.starts_with("Replace with Effect."),
        &[
            FixCompileCase { fixture: "failCauseToConstructor.ts", matches: 24, fixes: 22 },
            FixCompileCase { fixture: "failCauseToConstructor_preview.ts", matches: 1, fixes: 1 },
            FixCompileCase {
                fixture: "failCauseToConstructor_namedImports.ts",
                matches: 3,
                fixes: 0,
            },
            FixCompileCase {
                fixture: "failCauseToConstructor_aliasedEffect.ts",
                matches: 2,
                fixes: 2,
            },
            FixCompileCase {
                fixture: "failCauseToConstructor_lazyObjectLiteral.ts",
                matches: 1,
                fixes: 1,
            },
        ],
    );
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestFailCauseToConstructorFixesCompile".to_string(),
        test_fail_cause_to_constructor_fixes_compile,
    )]
}
