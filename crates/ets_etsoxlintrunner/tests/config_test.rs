use std::sync::Arc;

use ets_shim::core::tspath;
use ets_shim::tsoptions::{
    ParseConfigHost, new_tsconfig_source_file_from_file_path,
    parse_json_source_file_config_file_content,
};
use ets_shim::vfs::{FS, vfstest};

// go test: etsoxlintrunner/config_test.go:14 configHost 9678fc89559eafed
struct ConfigHost {
    fs: Arc<dyn FS>,
}

impl ParseConfigHost for ConfigHost {
    fn fs(&self) -> &dyn FS {
        &*self.fs
    }
    // Rust: Go's `tsoptions.NewDefaultParseConfigHost` has no current directory; the test passes absolute paths.
    fn get_current_directory(&self) -> &'static str {
        ""
    }
}

// go test: etsoxlintrunner/config_test.go:16 TestEffectFnOptionsWithExtends 78dcc53b0d49a94f
//
// Rust: the test package's blank import of etsoxlintrunner runs its `init`: `ets_etsoxlintrunner::init()` here. The
// host is leaked (tsrs takes `&'static dyn ParseConfigHost`).
#[test]
fn test_effect_fn_options_with_extends() {
    ets_etsoxlintrunner::init();
    let options = r#"{"plugins":[{"name":"@effect/language-service","effectFn":["span","inferred-span","suggested-span"]}]}"#;
    for (name, config, base) in [
        (
            "inline",
            format!(r#"{{"compilerOptions":{options},"files":["main.ts"]}}"#),
            "{}".to_string(),
        ),
        (
            "inherited",
            r#"{"extends":"./base.json","files":["main.ts"]}"#.to_string(),
            format!(r#"{{"compilerOptions":{options}}}"#),
        ),
        (
            "local",
            format!(
                r#"{{"extends":"./base.json","compilerOptions":{options},"files":["main.ts"]}}"#
            ),
            "{}".to_string(),
        ),
    ] {
        let fs: Arc<dyn FS> = Arc::new(vfstest::from_map(
            [
                ("/main.ts".to_string(), "export {}".to_string()),
                ("/base.json".to_string(), base),
                ("/tsconfig.json".to_string(), config.clone()),
            ],
            true,
        ));
        let host: &'static ConfigHost = Box::leak(Box::new(ConfigHost { fs }));
        let source = new_tsconfig_source_file_from_file_path(
            "/tsconfig.json",
            tspath::to_path("/tsconfig.json", "", true),
            &config,
        );
        let parsed = parse_json_source_file_config_file_content(
            source,
            host,
            "/",
            None,
            None,
            "/tsconfig.json",
            &[],
            None,
        );
        assert!(parsed.errors.is_empty(), "{name}: unexpected config errors: {:?}", parsed.errors);
        let Some(effect) = parsed.compiler_options().and_then(|o| o.effect.get()) else {
            panic!("{name}: Effect plugin options were lost");
        };
        let got = effect.effect_fn.clone();
        assert!(
            got.as_deref()
                == Some(&["span", "inferred-span", "suggested-span"].map(String::from)[..]),
            "{name}: unexpected effectFn options: {got:?}"
        );
    }
}
