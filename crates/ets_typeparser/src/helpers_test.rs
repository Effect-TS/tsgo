// Ported from effect-tsgo's internal/typeparser/helpers_test.go. The Effect fixture packages are mounted from the
// installed fixture tree (`testdata/tests/<version>/node_modules`, the tree internal/bundledeffect embeds), read in
// place by `mount_effect` (see there).

use std::mem::ManuallyDrop;
use std::sync::{Arc, LazyLock, Once};

use ets_go::testing::{self, T};
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::Type;
use ets_shim::compiler::{CheckerHandle, Program, ProgramOptions, ProgramRegions};
use ets_shim::core::arena::Region;
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{
    CompilerOptions, ModuleKind, ModuleResolutionKind, NewLineKind, P, ScriptTarget, Tristate,
};
use ets_shim::vfs::vfstest::MapFile;
use ets_shim::vfs::{FS, bundled, vfstest};
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

use crate::*;

// go test: internal/typeparser/checked_expression_types_init_test.go:5 init a216750304cb260f
// Rust: Go's test-binary `init`; every program-building helper runs it once before compiling.
fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        ets_shim::checker::register_expression_checked_callback(record_checked_expression_type);
    });
}

/// Rust-only: Go's `map[string]any` test file system (path -> file).
pub(crate) type TestFS = FxHashMap<String, MapFile>;

/// Rust-only: a checker and the program it belongs to (Go returns the checker and reads `c.Program()`), with what
/// the program allocated, so that dropping it frees the program as Go's GC does (`porting/TSRS_API.md`, "The API
/// path", step 6). The checker handle is dropped first (Go's `done`), then the program with its checkers (and
/// their `EffectLinks`), then its regions.
pub(crate) struct Compiled {
    pub(crate) program: &'static Program,
    pub(crate) checker: ManuallyDrop<CheckerHandle>,
    _loader_regions: Arc<ProgramRegions>,
    _region: Region,
}

impl Compiled {
    pub(crate) fn type_parser(&mut self) -> TypeParser<'_> {
        new_type_parser(self.program, &mut self.checker)
    }
}

impl Drop for Compiled {
    fn drop(&mut self) {
        // SAFETY: the handle is dropped once, here, and not used afterwards.
        unsafe { ManuallyDrop::drop(&mut self.checker) };
        // SAFETY: a program from `new_program` that never shared data with another version; the test is done with
        // everything derived from it (the parser borrows this value, so it is gone too).
        unsafe { ets_shim::compiler::free_unshared_program(self.program) };
    }
}

/// Rust-only: Go's `compiler.NewProgram(compiler.ProgramOptions{Config, Host, SingleThreaded})` over `testfs`,
/// in regions (`Compiled`), and the type checker Go takes with `program.GetTypeChecker`.
pub(crate) fn new_compiled(
    testfs: TestFS,
    compiler_options: CompilerOptions,
    file_names: Vec<String>,
) -> Compiled {
    init();
    let region = Region::new_scratch(1 << 20);
    let loader_regions = Arc::new(ProgramRegions::default());
    let program = {
        let _scope = region.enter();
        let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(testfs, true)));
        let host =
            ets_shim::compiler::new_compiler_host("/", fs, &bundled::lib_path(), None, None, None);
        let config = ets_shim::tsoptions::new_parsed_command_line(
            P::new(compiler_options),
            file_names,
            Vec::new(),
            ComparePathsOptions {
                use_case_sensitive_file_names: true,
                current_directory: "/".into(),
            },
        );
        let mut options = ProgramOptions::new(P::new(config), host);
        options.single_threaded = Tristate::True;
        options.checker_regions = true;
        options.checker_diagnostic_regions = true;
        options.regions = Some(Arc::clone(&loader_regions));
        ets_shim::compiler::new_program(options)
    };
    let checker = program.get_type_checker(&Context::background());
    Compiled {
        program,
        checker: ManuallyDrop::new(checker),
        _loader_regions: loader_regions,
        _region: region,
    }
}

/// Rust-only: the compiler options of Go's test helpers.
pub(crate) fn test_compiler_options() -> CompilerOptions {
    CompilerOptions {
        new_line: NewLineKind::LF,
        skip_default_lib_check: Tristate::True,
        no_error_truncation: Tristate::True,
        target: ScriptTarget::ES_NEXT,
        module: ModuleKind::NODE_NEXT,
        module_resolution: ModuleResolutionKind::NODE_NEXT,
        strict: Tristate::True,
        ..Default::default()
    }
}

// go test: internal/typeparser/helpers_test.go:20 compileAndGetCheckerAndSourceFilesInternal 0624adbb43087f17
// Rust: the checker handle's drop is Go's `done` (see `Compiled`).
#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "Go: strings.HasSuffix(path, \".ts\")"
)]
pub(crate) fn compile_and_get_checker_and_source_files_internal(
    t: &T,
    sources: &[(&str, &str)],
) -> (Compiled, FxHashMap<String, P<SourceFile>>) {
    t.helper();

    let mut testfs = TestFS::default();
    let mut file_names = Vec::with_capacity(sources.len());
    for &(path, source) in sources {
        testfs.insert(path.to_string(), MapFile::from(source));
        if !path.contains("/node_modules/") && path.ends_with(".ts") {
            file_names.push(path.to_string());
        }
    }

    let compiled = new_compiled(testfs, test_compiler_options(), file_names.clone());
    let mut source_files = FxHashMap::default();
    for file_name in file_names {
        let Some(sf) = compiled.program.get_source_file(&file_name) else {
            t.fatal(ets_go::sprintf!("failed to get source file %s", file_name));
        };
        source_files.insert(file_name, sf);
    }
    (compiled, source_files)
}

// go test: internal/typeparser/helpers_test.go:69 compileAndGetCheckerAndSourceFileInternal ead6360c85b7fffc
// compileAndGetCheckerAndSourceFileInternal is a copy of compileAndGetCheckerAndSourceFile
// for use in internal (package typeparser) tests that need access to unexported helpers.
pub(crate) fn compile_and_get_checker_and_source_file_internal(
    t: &T,
    source: &str,
) -> (Compiled, P<SourceFile>) {
    t.helper();

    let mut testfs = TestFS::default();
    testfs.insert("/.src/test.ts".to_string(), MapFile::from(source));

    let compiled = new_compiled(testfs, test_compiler_options(), vec!["/.src/test.ts".to_string()]);
    let Some(sf) = compiled.program.get_source_file("/.src/test.ts") else {
        t.fatal("Failed to get source file");
    };
    (compiled, sf)
}

/// Rust-only: Go's `bundledeffect.EffectVersion` (the fixture profile directory name).
pub(crate) type EffectVersion = &'static str;
pub(crate) const EFFECT_V3: EffectVersion = "effect-v3";
pub(crate) const EFFECT_V4: EffectVersion = "effect-v4";

/// Rust-only: the repository root (Go's `bundledeffect.EffectTsGoRootPath`).
fn effect_tsgo_root_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Rust-only: one fixture package's files, keyed by their mount path (`/node_modules/<package>/...`).
type PackageFiles = Arc<Vec<(String, Arc<[u8]>)>>;

/// Rust-only: Go's `packageFSCache`: each package of each profile is read once per test binary.
static PACKAGE_FS_CACHE: LazyLock<Mutex<FxHashMap<(EffectVersion, String), PackageFiles>>> =
    LazyLock::new(Mutex::default);

/// Rust-only: the files of `package_name` in the `version` fixture profile. Go reads them from the embedded
/// `testfixtures.tar.gz`, which `repoctl` builds from `testdata/tests/<version>/node_modules/<package>` (symbolic
/// links followed, regular files only, `repoctl/src/fixtures.ts`); `pnpm setup-repo` installs that tree, so it
/// is read here in place.
fn package_files(version: EffectVersion, package_name: &str) -> Result<PackageFiles, String> {
    let key = (version, package_name.to_string());
    if let Some(files) = PACKAGE_FS_CACHE.lock().get(&key) {
        return Ok(Arc::clone(files));
    }
    let root = effect_tsgo_root_path().join("testdata/tests").join(version).join("node_modules");
    let package_root = root.join(package_name);
    let mut files = Vec::new();
    fn collect(
        directory: &std::path::Path,
        mount: &str,
        files: &mut Vec<(String, Arc<[u8]>)>,
    ) -> Result<(), String> {
        let mut children: Vec<_> = std::fs::read_dir(directory)
            .map_err(|err| format!("{}: {err}", directory.display()))?
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        children.sort();
        for child in children {
            let source = directory.join(&child);
            let Ok(info) = std::fs::metadata(&source) else {
                continue;
            };
            let mount_path = format!("{mount}/{child}");
            if info.is_dir() {
                collect(&source, &mount_path, files)?;
            } else if info.is_file() {
                let content =
                    std::fs::read(&source).map_err(|err| format!("{}: {err}", source.display()))?;
                files.push((mount_path, Arc::from(content)));
            }
        }
        Ok(())
    }
    if !package_root.is_dir() {
        return Err(format!("package not installed at {}", package_root.display()));
    }
    collect(&package_root, &format!("/node_modules/{package_name}"), &mut files)?;
    let files = Arc::new(files);
    PACKAGE_FS_CACHE.lock().insert(key, Arc::clone(&files));
    Ok(files)
}

/// Rust-only: Go's `bundledeffect.EnsurePackageInstalled` (a test-support package of its own, not ported yet), over
/// the fixture tree `mount_effect` reads in place: an error when the package directory is missing or empty.
pub(crate) fn ensure_package_installed(
    version: EffectVersion,
    package_name: &str,
) -> Result<(), String> {
    let path = effect_tsgo_root_path()
        .join("testdata/tests")
        .join(version)
        .join("node_modules")
        .join(package_name);
    let installed = std::fs::read_dir(&path).is_ok_and(|mut entries| entries.next().is_some());
    if !installed {
        return Err(format!("package not installed at {}", path.display()));
    }
    Ok(())
}

/// Rust-only: Go's `bundledeffect.MountEffect` (a test-support package of its own, not ported yet), over the
/// fixture tree read in place (see `package_files`).
pub(crate) fn mount_effect(version: EffectVersion, testfs: &mut TestFS) -> Result<(), String> {
    let packages = ["effect", "pure-rand", "@standard-schema/spec", "fast-check", "@types/node"];
    for package_name in packages {
        for (path, content) in package_files(version, package_name)?.iter() {
            testfs.insert(path.clone(), MapFile::from(content.to_vec()));
        }
    }

    let package_json_path =
        effect_tsgo_root_path().join("testdata/tests").join(version).join("package.json");
    let package_json = std::fs::read(&package_json_path)
        .map_err(|_| format!("package.json not installed at {version}/package.json"))?;
    testfs.insert("/.src/package.json".to_string(), MapFile::from(package_json));
    Ok(())
}

// go test: internal/typeparser/helpers_test.go:109 compileAndGetCheckerAndSourceFileWithEffectV4Internal f847741f4e78a343
pub(crate) fn compile_and_get_checker_and_source_file_with_effect_v4_internal(
    t: &T,
    source: &str,
) -> (Compiled, P<SourceFile>) {
    compile_and_get_checker_and_source_file_with_effect_version_internal(t, EFFECT_V4, source)
}

// go test: internal/typeparser/helpers_test.go:113 compileAndGetCheckerAndSourceFileWithEffectV3Internal 9a882273dab795a8
pub(crate) fn compile_and_get_checker_and_source_file_with_effect_v3_internal(
    t: &T,
    source: &str,
) -> (Compiled, P<SourceFile>) {
    compile_and_get_checker_and_source_file_with_effect_version_internal(t, EFFECT_V3, source)
}

// go test: internal/typeparser/helpers_test.go:117 compileAndGetCheckerAndSourceFileWithEffectVersionInternal e3bc086beeefc6c0
pub(crate) fn compile_and_get_checker_and_source_file_with_effect_version_internal(
    t: &T,
    version: EffectVersion,
    source: &str,
) -> (Compiled, P<SourceFile>) {
    t.helper();

    let mut testfs = TestFS::default();
    testfs.insert("/.src/test.ts".to_string(), MapFile::from(source));
    if let Err(err) = mount_effect(version, &mut testfs) {
        t.fatal(ets_go::sprintf!("failed to mount effect %s: %v", version, err));
    }

    let compiled = new_compiled(testfs, test_compiler_options(), vec!["/.src/test.ts".to_string()]);
    let Some(sf) = compiled.program.get_source_file("/.src/test.ts") else {
        t.fatal("Failed to get source file");
    };
    (compiled, sf)
}

// go test: internal/typeparser/helpers_test.go:160 findIdentifierByText aee8d87e464c7b18
pub(crate) fn find_identifier_by_text(
    t: &T,
    sf: P<SourceFile>,
    text: &str,
    occurrence: usize,
) -> P<Node> {
    t.helper();

    fn visit(
        node: P<Node>,
        text: &str,
        occurrence: usize,
        count: &mut usize,
        found: &mut Option<P<Node>>,
    ) {
        if found.is_some() {
            return;
        }
        if node.kind() == Kind::Identifier && node.text() == text {
            if *count == occurrence {
                *found = Some(node);
                return;
            }
            *count += 1;
        }
        node.for_each_child(&mut |child| {
            visit(child, text, occurrence, count, found);
            false
        });
    }

    let mut count = 0;
    let mut found = None;
    visit(sf.as_node(), text, occurrence, &mut count, &mut found);
    let Some(found) = found else {
        t.fatal(ets_go::sprintf!("identifier %q occurrence %d not found", text, occurrence as i64));
    };
    found
}

// go test: internal/typeparser/helpers_test.go:196 getFirstVariableDeclarationType a8dd86bf1e4230db
// getFirstVariableDeclarationType finds the first variable declaration in the source file
// and returns its type and the declaration node (for use as atLocation).
fn get_first_variable_declaration_type(
    t: &T,
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> (Option<P<Type>>, P<Node>) {
    t.helper();

    let mut queue = std::collections::VecDeque::from([sf.as_node()]);
    while let Some(node) = queue.pop_front() {
        if node.kind() == Kind::VariableDeclaration
            && let Some(name_node) = node.name()
        {
            let typ = tp.get_type_at_location(name_node);
            return (typ, node);
        }
        node.for_each_child(&mut |child| {
            queue.push_back(child);
            false
        });
    }

    t.fatal("No variable declaration found in source file");
}

/// Rust-only: runs a ported test on a checker-sized stack (notes/port-tsrs.md, "Stack").
pub(crate) fn run(f: impl FnOnce(&T) + Send + 'static) {
    ets_go::testing::on_checker_stack(move || testing::run(f));
}

pub(crate) fn type_string(tp: &mut TypeParser<'_>, t: P<Type>) -> String {
    tp.checker().type_to_string_exported(t)
}

// go test: internal/typeparser/helpers_test.go:224 TestExtractCovariantType_RejectsGenericSignature 61d7e6c3e2cfab87
#[test]
fn test_extract_covariant_type_rejects_generic_signature() {
    run(|t| {
        t.parallel();

        let source = "declare const test: { prop: <T>() => T }";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let (typ, _) = get_first_variable_declaration_type(t, &mut tp, sf);

        let result = tp.extract_covariant_type(typ, "prop");
        if let Some(result) = result {
            t.error(ets_go::sprintf!(
                "expected nil for generic covariant signature, got %s",
                type_string(&mut tp, result)
            ));
        }
    });
}

// go test: internal/typeparser/helpers_test.go:240 TestExtractContravariantType_RejectsGenericSignature bebbb6f66b8fcc11
#[test]
fn test_extract_contravariant_type_rejects_generic_signature() {
    run(|t| {
        t.parallel();

        let source = "declare const test: { prop: <T>(_: T) => void }";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let (typ, _) = get_first_variable_declaration_type(t, &mut tp, sf);

        let result = tp.extract_contravariant_type(typ, "prop");
        if let Some(result) = result {
            t.error(ets_go::sprintf!(
                "expected nil for generic contravariant signature, got %s",
                type_string(&mut tp, result)
            ));
        }
    });
}

// go test: internal/typeparser/helpers_test.go:256 TestExtractCovariantType_AcceptsNonGenericSignature 978435398d81b2e0
#[test]
fn test_extract_covariant_type_accepts_non_generic_signature() {
    run(|t| {
        t.parallel();

        let source = "declare const test: { prop: () => string }";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let (typ, _) = get_first_variable_declaration_type(t, &mut tp, sf);

        let Some(result) = tp.extract_covariant_type(typ, "prop") else {
            t.fatal("expected non-nil result for non-generic covariant signature");
        };

        let result_str = type_string(&mut tp, result);
        if result_str != "string" {
            t.error(ets_go::sprintf!("expected return type 'string', got %q", result_str));
        }
    });
}

// go test: internal/typeparser/helpers_test.go:277 TestExtractInvariantType_RejectsGenericSignature 765bea23bdb06907
#[test]
fn test_extract_invariant_type_rejects_generic_signature() {
    run(|t| {
        t.parallel();

        let source = "declare const test: { prop: <T>(_: T) => T }";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let (typ, _) = get_first_variable_declaration_type(t, &mut tp, sf);

        let result = tp.extract_invariant_type(typ, "prop");
        if let Some(result) = result {
            t.error(ets_go::sprintf!(
                "expected nil for generic invariant signature, got %s",
                type_string(&mut tp, result)
            ));
        }
    });
}

// go test: internal/typeparser/helpers_test.go:293 TestExtractInvariantType_AcceptsNonGenericSignature 7f475d4493c94044
#[test]
fn test_extract_invariant_type_accepts_non_generic_signature() {
    run(|t| {
        t.parallel();

        let source = "declare const test: { prop: (_: string) => string }";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let (typ, _) = get_first_variable_declaration_type(t, &mut tp, sf);

        let Some(result) = tp.extract_invariant_type(typ, "prop") else {
            t.fatal("expected non-nil result for non-generic invariant signature");
        };

        let result_str = type_string(&mut tp, result);
        if result_str != "string" {
            t.error(ets_go::sprintf!("expected return type 'string', got %q", result_str));
        }
    });
}

// go test: internal/typeparser/helpers_test.go:314 TestExtractContravariantType_AcceptsNonGenericSignature 1d875b6ab6b3d615
#[test]
fn test_extract_contravariant_type_accepts_non_generic_signature() {
    run(|t| {
        t.parallel();

        let source = "declare const test: { prop: (_: string) => void }";

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(t, source);
        let mut tp = compiled.type_parser();

        let (typ, _) = get_first_variable_declaration_type(t, &mut tp, sf);

        let Some(result) = tp.extract_contravariant_type(typ, "prop") else {
            t.fatal("expected non-nil result for non-generic contravariant signature");
        };

        let result_str = type_string(&mut tp, result);
        if result_str != "string" {
            t.error(ets_go::sprintf!("expected parameter type 'string', got %q", result_str));
        }
    });
}

// go test: internal/typeparser/helpers_test.go:335 TestGetSymbolIfSameReference_UsesCanonicalExportSymbol 29cfa5202f971ce8
#[test]
fn test_get_symbol_if_same_reference_uses_canonical_export_symbol() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_internal(t, "export const Foo = 1");
        let mut tp = compiled.type_parser();

        let Some(local_foo) = tp.get_symbol_at_location(find_identifier_by_text(t, sf, "Foo", 0))
        else {
            t.fatal("expected local Foo symbol");
        };

        let Some(module_sym) = get_symbol_of_declaration(tp.checker, sf.as_node()) else {
            t.fatal("expected module symbol");
        };

        let Some(export_foo) =
            tp.checker.try_get_member_in_module_exports_and_properties("Foo", module_sym)
        else {
            t.fatal("expected exported Foo symbol");
        };

        if get_symbol_if_same_reference(tp.checker, Some(local_foo), Some(export_foo)).is_none() {
            t.fatal("expected local and exported Foo symbols to match");
        }
    });
}

// go test: internal/typeparser/helpers_test.go:361 TestGetSymbolIfSameReference_ResolvesImportAliases 9141069b2f36a790
#[test]
fn test_get_symbol_if_same_reference_resolves_import_aliases() {
    run(|t| {
        t.parallel();

        let (mut compiled, source_files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[
                ("/.src/a.ts", "export const Foo = 1"),
                ("/.src/b.ts", "import { Foo as Bar } from \"./a\"\nconst value = Bar"),
            ],
        );
        let mut tp = compiled.type_parser();

        let (Some(&a), Some(&b)) = (source_files.get("/.src/a.ts"), source_files.get("/.src/b.ts"))
        else {
            t.fatal("expected both source files");
        };

        let Some(module_sym) = get_symbol_of_declaration(tp.checker, a.as_node()) else {
            t.fatal("expected module symbol for a.ts");
        };

        let Some(export_foo) =
            tp.checker.try_get_member_in_module_exports_and_properties("Foo", module_sym)
        else {
            t.fatal("expected exported Foo symbol");
        };

        let Some(bar) = tp.get_symbol_at_location(find_identifier_by_text(t, b, "Bar", 0)) else {
            t.fatal("expected alias symbol for Bar");
        };

        if get_symbol_if_same_reference(tp.checker, Some(export_foo), Some(bar)).is_none() {
            t.fatal("expected exported Foo and imported Bar symbols to match");
        }
        if get_symbol_if_same_reference(tp.checker, Some(bar), Some(export_foo)).is_none() {
            t.fatal("expected symbol comparison to be symmetric");
        }
    });
}

// go test: internal/typeparser/helpers_test.go:401 TestGetSymbolIfSameReference_DoesNotMatchDifferentExports e7545da97325c0e0
#[test]
fn test_get_symbol_if_same_reference_does_not_match_different_exports() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            "export const Foo = 1\nexport const Bar = 2",
        );
        let mut tp = compiled.type_parser();

        let foo = tp.get_symbol_at_location(find_identifier_by_text(t, sf, "Foo", 0));
        let bar = tp.get_symbol_at_location(find_identifier_by_text(t, sf, "Bar", 0));
        if foo.is_none() || bar.is_none() {
            t.fatal("expected Foo and Bar symbols");
        }

        if get_symbol_if_same_reference(tp.checker, foo, bar).is_some() {
            t.fatal("expected different exports not to match");
        }
    });
}
