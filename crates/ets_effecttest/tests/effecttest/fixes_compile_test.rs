use ets_bundledeffect::EffectVersion;
use ets_fourslash::{FourslashTest, QuickFixInfo};
use ets_go::testing::T;
use ets_shim::ls::lsconv;

use crate::has_diagnostic_code;

// go test: internal/effecttest/fixes_compile_test.go:16 fixCompileCase 9312f9f9cc78cbfd
pub(crate) struct FixCompileCase {
    pub(crate) fixture: &'static str,
    pub(crate) matches: usize,
    pub(crate) fixes: usize,
}

// go test: internal/effecttest/fixes_compile_test.go:22 fixesCompileFile 799e27ee1f664969
#[derive(Clone)]
struct FixesCompileFile {
    name: String,
    content: String,
}

// go test: internal/effecttest/fixes_compile_test.go:27 fixesCompileMatch 29b6e6b63c57d14c
struct FixesCompileMatch {
    file: usize,
    info: QuickFixInfo,
}

// go test: internal/effecttest/fixes_compile_test.go:32 verifyFixesCompile bc03a37a47cb4fdf
pub(crate) fn verify_fixes_compile(
    t: &T,
    version: EffectVersion,
    rule_name: &str,
    code: i32,
    is_fix: impl Fn(&str) -> bool,
    cases: &[FixCompileCase],
) {
    t.helper();
    let prefix = format!(
        r#"// @filename: /tsconfig.json
{{
  "compilerOptions": {{
    "strict": true,
    "skipLibCheck": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [{{
      "name": "@effect/language-service",
      "diagnosticSeverity": {{ "*": "off", "promiseInEffectSuccess": "off", "{rule_name}": "suggestion" }}
    }}]
  }}
}}
"#
    );
    let compile = |t: &T, files: &[FixesCompileFile]| -> (FourslashTest, Vec<FixesCompileMatch>) {
        t.helper();
        let mut sb = String::new();
        sb.push_str(&prefix);
        for file in files {
            sb.push_str("// @filename: ");
            sb.push_str(&file.name);
            sb.push('\n');
            sb.push_str(&file.content);
        }
        let mut f = ets_fourslash::new_fourslash(t, None, &sb);
        let mut matches = Vec::new();
        for (index, file) in files.iter().enumerate() {
            f.go_to_file(t, &file.name);
            f.verify_non_suggestion_diagnostics(t, &[]);
            for info in
                f.get_quick_fixes_for_diagnostics(t, &lsconv::file_name_to_document_uri(&file.name))
            {
                if has_diagnostic_code(&info.diagnostic, code) {
                    matches.push(FixesCompileMatch { file: index, info });
                }
            }
        }
        (f, matches)
    };
    for test in cases {
        t.run(&format!("{}/{}", version.as_str(), test.fixture), |t| {
            t.parallel();
            let content = match std::fs::read_to_string(
                ets_effecttest::test_data_path()
                    .join("tests")
                    .join(version.as_str())
                    .join(test.fixture),
            ) {
                Ok(content) => content,
                Err(err) => t.fatal(err.to_string()),
            };
            let mut source = demote_rule_to_suggestion(&content, rule_name);
            if !source.starts_with("// @filename:") {
                source = format!("// @filename: repro.ts\n{source}");
            }
            let mut files: Vec<FixesCompileFile> = Vec::new();
            for line in source.split_inclusive('\n') {
                if let Some(name) = line.strip_prefix("// @filename:") {
                    files.push(FixesCompileFile {
                        name: format!("/{}", name.trim()),
                        content: format!("// @{}\n", version.as_str()),
                    });
                    continue;
                }
                let last = files.len() - 1;
                files[last].content.push_str(line);
            }
            let (mut f, matches) = compile(t, &files);
            let mut fixes = 0;
            for m in &matches {
                let uri = lsconv::file_name_to_document_uri(&files[m.file].name);
                for (index, title) in m.info.fix_titles.iter().enumerate() {
                    if !is_fix(title) {
                        continue;
                    }
                    let result = f.apply_quick_fix(t, &uri, &m.info.diagnostic, index);
                    if result.changes.len() != 1 || result.changes[0].uri != uri {
                        t.fatal(format!(
                            "expected one change in {}, got {}",
                            uri.0,
                            result.changes.len()
                        ));
                    }
                    let mut fixed = files.clone();
                    fixed[m.file].content.clone_from(&result.changes[0].after);
                    {
                        let (mut close_fixed, remaining) = compile(t, &fixed);
                        if remaining.len() != matches.len() - 1 {
                            t.fatal(format!(
                                "expected {} diagnostics after {title:?}, got {}",
                                matches.len() - 1,
                                remaining.len()
                            ));
                        }
                        close_fixed.done(t);
                    }
                    fixes += 1;
                }
            }
            if matches.len() != test.matches || fixes != test.fixes {
                t.fatal(format!(
                    "got {} diagnostics and {fixes} compiled fixes, want {} and {}",
                    matches.len(),
                    test.matches,
                    test.fixes
                ));
            }
            f.done(t);
        });
    }
}

// go test: internal/effecttest/fixes_compile_test.go:120 demoteRuleToSuggestion 7e8bcb45286a6f52
fn demote_rule_to_suggestion(source: &str, rule_name: &str) -> String {
    source.replace(&format!("{rule_name}:warning"), &format!("{rule_name}:suggestion"))
}
