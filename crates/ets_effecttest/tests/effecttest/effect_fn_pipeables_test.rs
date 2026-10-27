// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_go::testing::T;
use ets_shim::ls::lsconv;
use libtest_mimic::Trial;

use crate::run_case;

// go test: internal/effecttest/effect_fn_pipeables_test.go:15 TestEffectFnOpportunityUnaryPipeablesCompile cc847fba8f8c4009
fn test_effect_fn_opportunity_unary_pipeables_compile(t: &T) {
    const PREFIX: &str = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [{
      "name": "@effect/language-service",
      "effectFn": ["span", "untraced", "no-span", "inferred-span", "suggested-span"]
    }]
  }
}
// @Filename: /repro.ts
"#;
    ets_effecttest::init_hooks();
    t.parallel();

    let fixture = match std::fs::read_to_string(
        ets_effecttest::test_data_path()
            .join("tests")
            .join("effect-v4")
            .join("effectFnOpportunity_unaryPipeables.ts"),
    ) {
        Ok(fixture) => fixture,
        Err(err) => t.fatal(err.to_string()),
    };
    let Some((_, source)) =
        fixture.split_once("// @filename: effectFnOpportunity_unaryPipeables.ts\n")
    else {
        t.fatal("missing fixture source");
    };
    let mut f = ets_fourslash::new_fourslash(t, None, &format!("{PREFIX}{source}"));
    f.verify_non_suggestion_diagnostics(t, &[]);

    let uri = lsconv::file_name_to_document_uri("/repro.ts");
    let mut checked = 0;
    for diagnostic in f.get_quick_fixes_for_diagnostics(t, &uri) {
        for (index, title) in diagnostic.fix_titles.iter().enumerate() {
            if !title.starts_with("Convert to Effect.fn") {
                continue;
            }
            let result = f.apply_quick_fix(t, &uri, &diagnostic.diagnostic, index);
            if result.changes.len() != 1 {
                t.fatal(format!(
                    "{title}: expected one changed file, got {}",
                    result.changes.len()
                ));
            }
            t.log(format!("Checking {title} at {:?}", diagnostic.diagnostic.range));
            {
                let mut converted = ets_fourslash::new_fourslash(
                    t,
                    None,
                    &format!("{PREFIX}{}", result.changes[0].after),
                );
                converted.verify_non_suggestion_diagnostics(t, &[]);
                converted.done(t);
            }
            checked += 1;
        }
    }
    if checked == 0 {
        t.fatal("no Effect.fn conversions were checked");
    }
    f.done(t);
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestEffectFnOpportunityUnaryPipeablesCompile".to_string(),
        test_effect_fn_opportunity_unary_pipeables_compile,
    )]
}
