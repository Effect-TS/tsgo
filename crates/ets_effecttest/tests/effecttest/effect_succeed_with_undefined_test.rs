// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/effect_succeed_with_undefined_test.go:11 TestEffectSucceedWithUndefinedFixesCompile 7eb628f2869aa2d2
fn test_effect_succeed_with_undefined_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let code = tsdiag::EFFECT_UNDEFINED_REPRESENTS_THE_SAME_OUTCOME_AS_EFFECT_SUCCEED_UNDEFINED_OR_EFFECT_SUCCEED_VOID_0_EFFECT_EFFECT_SUCCEED_WITH_UNDEFINED.code();
    verify_fixes_compile(
        t,
        EffectVersion::V4,
        "effectSucceedWithUndefined",
        code,
        |title| title.starts_with("Replace with Effect.undefined"),
        &[
            FixCompileCase { fixture: "effectSucceedWithUndefined.ts", matches: 15, fixes: 15 },
            FixCompileCase {
                fixture: "effectSucceedWithUndefined_aliases.ts",
                matches: 12,
                fixes: 12,
            },
            FixCompileCase {
                fixture: "effectSucceedWithUndefined_preview.ts",
                matches: 1,
                fixes: 1,
            },
            FixCompileCase {
                fixture: "effectSucceedWithUndefined_namedImport.ts",
                matches: 3,
                fixes: 0,
            },
            FixCompileCase {
                fixture: "effectSucceedWithUndefined_scopedEffect.ts",
                matches: 4,
                fixes: 2,
            },
            FixCompileCase {
                fixture: "effectSucceedWithUndefined_callShapes.ts",
                matches: 5,
                fixes: 5,
            },
            FixCompileCase {
                fixture: "effectSucceedWithUndefined_namedUndefinedImport.ts",
                matches: 1,
                fixes: 1,
            },
        ],
    );
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestEffectSucceedWithUndefinedFixesCompile".to_string(),
        test_effect_succeed_with_undefined_fixes_compile,
    )]
}
