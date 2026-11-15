// Paired base-constraint regression for oxc-project/tsrs#996.
// The paired Go test uses the same fixtures and sequential retained-checker histories.
use std::sync::{Arc, Mutex};

use ets_shim::P;
use ets_shim::compiler::{
    CheckerHandle, CheckerPool, CompilerHost, Context, PooledChecker, Program, ProgramOptions,
    new_compiler_host, new_program,
};
use ets_shim::tsoptions::{self, ParseConfigHost};
use ets_shim::vfs::{FS, bundled, vfstest};

const A: &str = include_str!("../../../testdata/compiler/base-constraint-history/a.ts");
const B: &str = include_str!("../../../testdata/compiler/base-constraint-history/b.ts");
const LONE: &str = include_str!("../../../testdata/compiler/base-constraint-history/lone.ts");
const CYCLE: &str = include_str!("../../../testdata/compiler/base-constraint-history/cycle.ts");

struct ConfigHost {
    fs: Arc<dyn FS>,
}
impl ParseConfigHost for ConfigHost {
    fn fs(&self) -> &dyn FS {
        &*self.fs
    }
    fn get_current_directory(&self) -> &'static str {
        "/src"
    }
}

struct RetainedPool {
    program: &'static Program,
    idle: Arc<Mutex<Option<PooledChecker>>>,
}
impl CheckerPool for RetainedPool {
    fn get_checker(&self, _: &Context, _: Option<P<ets_shim::ast::SourceFile>>) -> CheckerHandle {
        let mut checker =
            self.idle.lock().unwrap().take().unwrap_or_else(|| {
                PooledChecker::new(ets_shim::checker::new_checker(self.program))
            });
        let ptr = checker.as_non_null();
        let idle = Arc::clone(&self.idle);
        // SAFETY: sequential per-file requests own the checker exclusively until release.
        unsafe { CheckerHandle::from_raw(ptr, move || *idle.lock().unwrap() = Some(checker)) }
    }
}

#[test]
fn base_constraint_history_matrix() {
    // Keep process-wide checker state isolated from the rest of the integration binary.
    const CHILD: &str = "TSRS_BASE_CONSTRAINT_HISTORY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "base_constraint_history_matrix", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        assert!(output.status.success(), "native history regression failed");
        return;
    }
    let mut failures = Vec::new();
    for (case, a, b, expected) in [
        ("pair", A, B.to_string(), None),
        ("pair-both", A, B.replace("return x.b;", "return x.b + y.b;"), None),
        (
            "mapped-leftmost",
            A,
            format!(
                "type Mapped<T> = {{ [K in keyof T]: T[K] }};\n{}",
                B.replace("y: T[\"a\"]", "y: Mapped<T>[\"a\"]")
            ),
            None,
        ),
        (
            "intersection-leftmost",
            A,
            B.replace("y: T[\"a\"]", "y: (T & { marker?: never })[\"a\"]"),
            None,
        ),
        ("missing", A, B.replace("x.b;", "x.missing;"), Some((2339, "missing"))),
        ("lone", "", LONE.to_string(), None),
        ("wrong-return", "", LONE.replace("): string", "): number"), Some((2322, "return"))),
        ("cycle", "", CYCLE.to_string(), Some((2313, "T>("))),
        (
            "indexed-expansion",
            "",
            include_str!("../../../testdata/compiler/base-constraint-history/expanding.ts")
                .to_string(),
            Some((2589, "Expand<{ a: {} }>")),
        ),
    ] {
        let histories: &[&[&str]] = if a.is_empty() {
            &[&["b.ts"]]
        } else {
            &[&["a.ts", "b.ts"], &["b.ts", "a.ts"], &["b.ts"]]
        };
        for history in histories {
            let config = if a.is_empty() || history.len() == 1 {
                r#"{"compilerOptions":{"strict":true,"noEmit":true,"target":"es2020","types":[]},"files":["b.ts"]}"#
            } else {
                r#"{"compilerOptions":{"strict":true,"noEmit":true,"target":"es2020","types":[]},"files":["a.ts","b.ts"]}"#
            };
            let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(
                [("/src/tsconfig.json", config), ("/src/a.ts", a), ("/src/b.ts", b.as_str())],
                true,
            )));
            let parse_host = Box::leak(Box::new(ConfigHost { fs: Arc::clone(&fs) }));
            let (parsed, errors) = tsoptions::get_parsed_command_line_of_config_file(
                "/src/tsconfig.json",
                None,
                None,
                parse_host,
                None,
            );
            assert!(errors.is_empty(), "invalid fixture config");
            let host: Arc<dyn CompilerHost> =
                new_compiler_host("/src", fs, &bundled::lib_path(), None, None, None);
            let mut options = ProgramOptions::new(P::new(parsed.unwrap()), host);
            options.create_checker_pool = Some(Arc::new(|program| {
                Box::new(RetainedPool { program, idle: Arc::new(Mutex::new(None)) })
            }));
            let program = new_program(options);
            let mut observed = Vec::new();
            for file in *history {
                let source = program.get_source_file(&format!("/src/{file}")).unwrap();
                for d in program.get_semantic_diagnostics(&Context::default(), Some(source)) {
                    observed.push((
                        d.file().unwrap().file_name().to_string(),
                        d.code(),
                        d.pos(),
                        d.end(),
                    ));
                }
            }
            println!("case={case} history={history:?} diagnostics={observed:?}");
            let valid = match expected {
                None => observed.is_empty(),
                Some((code, token)) => {
                    let start = b.find(token).unwrap() as u32;
                    let end = start + if case == "cycle" { 1 } else { token.len() as u32 };
                    observed.len() == 1
                        && observed[0].0 == "/src/b.ts"
                        && observed[0].1 == code
                        && observed[0].2 == Some(start)
                        && observed[0].3 == Some(end)
                }
            };
            if !valid {
                failures.push(format!("{case}/{history:?}: {observed:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "type-derived expectations failed:\n{}", failures.join("\n"));
}
