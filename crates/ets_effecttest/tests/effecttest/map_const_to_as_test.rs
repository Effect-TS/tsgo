// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/map_const_to_as_test.go:10 TestMapConstToAsFixesCompile 4f4597e862777a26
fn test_map_const_to_as_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    for version in [EffectVersion::V3, EffectVersion::V4] {
        t.run(version.as_str(), |t| {
            t.parallel();
            verify_fixes_compile(
                t,
                version,
                "mapConstToAs",
                tsdiag::EFFECT_AS_EXPRESSES_REPLACING_THE_SUCCESS_VALUE_WITH_A_CONSTANT_DIRECTLY_EFFECT_MAP_CONST_TO_AS.code(),
                |title| title == "Replace with Effect.as",
                &[
                    FixCompileCase { fixture: "mapConstToAs.ts", matches: 23, fixes: 23 },
                    FixCompileCase { fixture: "mapConstToAs_fixShapes.ts", matches: 4, fixes: 4 },
                ],
            );
        });
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case("TestMapConstToAsFixesCompile".to_string(), test_map_const_to_as_fixes_compile)]
}
