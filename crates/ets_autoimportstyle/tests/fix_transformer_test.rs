// Rust-only: a regression test for the tsrs ports of patches 018 (`FixTransformer`,
// `RegisterAutoImportFixTransformer`, `View.applyStylePolicy`) and 027 (the factory call in `NewView`): the
// registered factory's transformer sees every fix `View::get_fixes` returns, and its result replaces them. It is a
// test binary of its own because the factory is process-wide.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ets_shim::ast::SymbolFlags;
use ets_shim::compiler::ProgramOptions;
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{
    CompilerOptions, ModuleKind, ModuleResolutionKind, P, ScriptTarget, Tristate,
};
use ets_shim::ls::autoimport::{
    Export, ExportID, ExportSyntax, Fix, FixTransformer, ModuleID, ProjectID, new_view,
    register_auto_import_fix_transformer,
};
use ets_shim::lsproto;
use ets_shim::modulespecifiers::UserPreferences;
use ets_shim::vfs::{FS, bundled, vfstest};

#[test]
fn test_registered_fix_transformer_rewrites_fixes() {
    let files = vec![
        ("/foo.ts".to_string(), "export const a = 1;\nexport const b = 2;\n".to_string()),
        ("/main.ts".to_string(), "import { a } from \"./foo\";\na;\nb;\n".to_string()),
    ];
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(files, true)));
    let compiler_options = P::new(CompilerOptions {
        target: ScriptTarget::ES_NEXT,
        module: ModuleKind::ES_NEXT,
        module_resolution: ModuleResolutionKind::BUNDLER,
        strict: Tristate::True,
        ..Default::default()
    });
    let host =
        ets_shim::compiler::new_compiler_host("/", fs, &bundled::lib_path(), None, None, None);
    let config = ets_shim::tsoptions::new_parsed_command_line(
        compiler_options,
        vec!["/foo.ts".to_string(), "/main.ts".to_string()],
        Vec::new(),
        ComparePathsOptions { use_case_sensitive_file_names: true, current_directory: "/".into() },
    );
    let mut options = ProgramOptions::new(P::new(config), host);
    options.single_threaded = Tristate::True;
    let program = ets_shim::compiler::new_program(options);
    let importing_file = program.get_source_file("/main.ts").expect("missing /main.ts");
    let mut checker = program.get_type_checker(&Context::background());

    // `Export` has crate-private fields: start from the default, as the shim integration does.
    let mut export = Export::default();
    export.export_id =
        ExportID { module_id: ModuleID::File("/foo.ts".to_string()), export_name: "b".to_string() };
    export.target = export.export_id.clone();
    export.syntax = ExportSyntax::Modifier;
    export.flags = SymbolFlags::BlockScopedVariable;

    // A factory that returns no transformer leaves the fixes alone.
    let factory_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&factory_calls);
    register_auto_import_fix_transformer(Arc::new(move |_, _, file| {
        assert_eq!(file.file_name(), "/main.ts");
        calls.fetch_add(1, Ordering::SeqCst);
        None
    }));
    let view = new_view(
        None,
        importing_file,
        ProjectID::default(),
        program,
        &mut checker,
        UserPreferences::default(),
    );
    let fixes = view.get_fixes(&export, false, true, None);
    assert_eq!(factory_calls.load(Ordering::SeqCst), 1, "NewView must call the factory once");
    assert_eq!(fixes.len(), 1, "expected the add-to-existing-import fix");
    assert_eq!(fixes[0].kind, lsproto::AutoImportFixKind::ADD_TO_EXISTING);
    drop(view);

    // A transformer replaces the fixes it is given.
    let seen = Arc::new(AtomicUsize::new(0));
    let seen_by_factory = Arc::clone(&seen);
    register_auto_import_fix_transformer(Arc::new(move |_, _, _| {
        let seen = Arc::clone(&seen_by_factory);
        let transformer: FixTransformer =
            Box::new(move |export: Option<&Export>, fixes: Vec<Arc<Fix>>| -> Vec<Arc<Fix>> {
                assert_eq!(export.map(|e| e.export_name.as_str()), Some("b"));
                seen.fetch_add(fixes.len(), Ordering::SeqCst);
                Vec::new()
            });
        Some(transformer)
    }));
    let view = new_view(
        None,
        importing_file,
        ProjectID::default(),
        program,
        &mut checker,
        UserPreferences::default(),
    );
    let fixes = view.get_fixes(&export, false, true, None);
    assert_eq!(seen.load(Ordering::SeqCst), 1, "the transformer must see the fix");
    assert!(fixes.is_empty(), "a nil transformer result suppresses the fixes");
}
