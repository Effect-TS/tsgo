use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::ast::Kind;
use ets_shim::core::context::{Context, Locale};
use ets_shim::core::tspath;
use ets_shim::diagnostics::Message;
use ets_shim::ls::{LanguageService, lsconv};
use ets_shim::lsproto;
use ets_shim::project::{self, logging};
use ets_shim::vfs::vfstest::MapFile;
use ets_shim::vfs::{FS, bundled, vfstest};

use crate::*;

// go test: internal/effecttest/document_symbols_runner.go:28 DocumentSymbolTestCasesDir 1c15f842604e9045
// DocumentSymbolTestCasesDir returns the path to the Effect document-symbol test cases directory.
pub fn document_symbol_test_cases_dir(version: EffectVersion) -> PathBuf {
    ets_bundledeffect::effect_ts_go_root_path()
        .join("testdata")
        .join("tests")
        .join(version.as_str().to_string() + "-document-symbols")
}

// go test: internal/effecttest/document_symbols_runner.go:33 DiscoverDocumentSymbolTestCases 240e86ca2155e622
// DiscoverDocumentSymbolTestCases finds all .ts test files in the document-symbol test cases directory.
pub fn discover_document_symbol_test_cases(
    version: EffectVersion,
) -> std::io::Result<Vec<PathBuf>> {
    let dir = document_symbol_test_cases_dir(version);
    let entries = match read_dir_sorted(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };

    let mut cases = Vec::new();
    for (name, is_dir) in entries {
        if !is_dir && Path::new(&name).extension().is_some_and(|e| e == "ts") {
            cases.push(dir.join(name));
        }
    }
    Ok(cases)
}

// go test: internal/effecttest/document_symbols_runner.go:53 RunEffectDocumentSymbolsTest 40856a281c071865
// RunEffectDocumentSymbolsTest executes one document-symbol baseline test case.
//
// Rust: Go never closes the session (the GC frees it); here it is closed at the end.
pub fn run_effect_document_symbols_test(t: &T, version: EffectVersion, test_file: &Path) {
    init_hooks();
    acquire_program();
    let _release = ReleaseProgramOnDrop;

    let content = match std::fs::read_to_string(test_file) {
        Ok(content) => content,
        Err(err) => t.fatal(format!("Failed to read test file: {err}")),
    };

    let default_file_name = tspath::get_base_file_name(&test_file.to_string_lossy());
    let units = parse_test_units(&content, &default_file_name);

    let mut testfs = ets_bundledeffect::TestFS::default();
    if let Err(err) = ets_bundledeffect::mount_effect(version, &mut testfs) {
        t.fatal(format!("Failed to mount Effect: {err}"));
    }

    let current_directory = "/.src";
    let mut source_file_names: Vec<String> = Vec::with_capacity(units.len());
    let mut has_ts_config = false;
    let mut config_directory = current_directory.to_string();

    for unit in &units {
        let unit_name = tspath::get_normalized_absolute_path(&unit.name, current_directory);
        testfs.insert(unit_name.clone(), MapFile::from(unit.content.as_str()));

        let lower_name = ets_go::strings::to_lower(&unit_name);
        if lower_name.ends_with("tsconfig.json") || lower_name.ends_with("jsconfig.json") {
            has_ts_config = true;
            continue;
        }
        if unit_name.starts_with("/node_modules/") {
            continue;
        }
        if source_file_names.is_empty() {
            config_directory = tspath::get_directory_path(&unit_name);
        }
        source_file_names.push(unit_name);
    }

    if !has_ts_config {
        let unit_name = tspath::combine_paths(&config_directory, &["tsconfig.json"]);
        testfs.insert(unit_name, MapFile::from(DEFAULT_TS_CONFIG));
    }

    // Rust-only: the source files' contents, read back below (Go reads them from the map).
    let contents: Vec<Option<String>> =
        source_file_names.iter().map(|name| read_map_file_content(testfs.get(name))).collect();

    let fs = vfstest::from_map(testfs, true);
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(fs));

    let session = project::new_session(&project::SessionInit {
        background_ctx: Context::background(),
        options: Arc::new(project::SessionOptions {
            current_directory: current_directory.to_string(),
            default_library_path: bundled::lib_path(),
            typings_location: "/home/src/Library/Caches/typescript".to_string(),
            position_encoding: lsproto::PositionEncodingKind::UTF8,
            watch_enabled: false,
            logging_enabled: false,
            ..Default::default()
        }),
        fs,
        client: Some(Arc::new(NoopProjectClient)),
        logger: Some(logging::new_logger(Box::new(std::io::sink()))),
        npm_executor: None,
        parse_cache: None,
    });

    for (file_name, content) in source_file_names.iter().zip(contents) {
        let Some(content) = content else {
            t.fatal(format!("missing file content for {file_name}"));
        };
        session.did_open_file(
            &Context::background(),
            lsconv::file_name_to_document_uri(file_name),
            1,
            content,
            lsproto::LanguageKind::TYPE_SCRIPT,
        );
    }

    let mut file_results: Vec<DocumentSymbolsFileResult> =
        Vec::with_capacity(source_file_names.len());
    for file_name in &source_file_names {
        file_results.push(collect_document_symbols_for_file(t, &session, file_name));
    }

    let base_file_name = tspath::get_base_file_name(&test_file.to_string_lossy());
    let baseline_name = base_file_name.strip_suffix(".ts").unwrap_or(&base_file_name).to_string();
    do_document_symbols_baseline(t, &baseline_name, version.as_str(), &file_results);

    // Rust-only: see the function's comment.
    session.close();
}

// go test: internal/effecttest/document_symbols_runner.go:134 collectDocumentSymbolsForFile 6b1e5d85c76e645b
pub(crate) fn collect_document_symbols_for_file(
    t: &T,
    session: &project::Session,
    file_name: &str,
) -> DocumentSymbolsFileResult {
    t.helper();

    let uri = lsconv::file_name_to_document_uri(file_name);
    let lang_service = match session.get_language_service(&Context::background(), &uri) {
        Ok(lang_service) => lang_service,
        Err(err) => {
            t.fatal(format!("failed to get language service for {file_name}: {}", err.message))
        }
    };

    let mut hierarchical = collect_hierarchical_document_symbols(t, &lang_service, &uri);
    let mut flat = collect_flat_document_symbols(t, &lang_service, &uri);

    // Native import symbols differ between compiler providers.
    let source_file = lang_service.get_program().get_source_file(file_name).unwrap();
    // Go's shim accessor `ls.LanguageService_converters` (tsrs patch 100).
    let converters = Arc::clone(&lang_service.converters);
    let is_import_range = |r: lsproto::Range| -> bool {
        let span = converters.from_lsp_range_to_original(&source_file, r);
        let (pos, end) = (span.pos().expect_source("a span"), span.end().expect_source("a span"));
        if pos >= end {
            return false;
        }
        for statement in source_file.statements().nodes() {
            if statement.kind() == Kind::ImportDeclaration
                && statement.pos().expect_source("a statement") <= pos
                && end <= statement.end().expect_source("a statement")
            {
                return true;
            }
        }
        false
    };
    hierarchical.retain(|symbol| !is_import_range(symbol.range));
    flat.retain(|symbol| {
        !(symbol.container_name.as_deref().is_none_or(str::is_empty)
            && is_import_range(symbol.location.range))
    });

    DocumentSymbolsFileResult {
        file_name: file_name.to_string(),
        document_trees: vec![hierarchical],
        flat_lists: vec![flat],
    }
}

// go test: internal/effecttest/document_symbols_runner.go:175 collectHierarchicalDocumentSymbols 7b340d89b3404811
pub(crate) fn collect_hierarchical_document_symbols(
    t: &T,
    lang_service: &LanguageService,
    uri: &lsproto::DocumentUri,
) -> Vec<lsproto::DocumentSymbol> {
    t.helper();

    let caps = lsproto::ClientCapabilities {
        text_document: Some(lsproto::TextDocumentClientCapabilities {
            document_symbol: Some(lsproto::DocumentSymbolClientCapabilities {
                hierarchical_document_symbol_support: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let resolved_caps = caps.resolve();
    let ctx = lsproto::with_client_capabilities(&Context::background(), Arc::new(resolved_caps));
    let result = match lang_service.provide_document_symbols(&ctx, uri) {
        Ok(result) => result,
        Err(err) => t.fatal(format!(
            "ProvideDocumentSymbols hierarchical failed for {}: {}",
            uri.0, err.message
        )),
    };
    result.document_symbols.unwrap_or_default()
}

// go test: internal/effecttest/document_symbols_runner.go:200 collectFlatDocumentSymbols d38d974e9affde9f
pub(crate) fn collect_flat_document_symbols(
    t: &T,
    lang_service: &LanguageService,
    uri: &lsproto::DocumentUri,
) -> Vec<lsproto::SymbolInformation> {
    t.helper();

    let caps = lsproto::ClientCapabilities {
        text_document: Some(lsproto::TextDocumentClientCapabilities {
            document_symbol: Some(lsproto::DocumentSymbolClientCapabilities {
                hierarchical_document_symbol_support: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let resolved_caps = caps.resolve();
    let ctx = lsproto::with_client_capabilities(&Context::background(), Arc::new(resolved_caps));
    let result = match lang_service.provide_document_symbols(&ctx, uri) {
        Ok(result) => result,
        Err(err) => {
            t.fatal(format!("ProvideDocumentSymbols flat failed for {}: {}", uri.0, err.message))
        }
    };
    result.symbol_informations.unwrap_or_default()
}

// go test: internal/effecttest/document_symbols_runner.go:225 readMapFileContent 98c99feeff1653d5
//
// Rust: Go's map holds `*fstest.MapFile`s or strings; the Rust test file system holds `MapFile`s only.
pub(crate) fn read_map_file_content(entry: Option<&MapFile>) -> Option<String> {
    let file = entry?;
    Some(String::from_utf8_lossy(&file.data).into_owned())
}

// go test: internal/effecttest/document_symbols_runner.go:236 noopProjectClient eb958a9c31a7b551
pub(crate) struct NoopProjectClient;

impl project::Client for NoopProjectClient {
    // go test: internal/effecttest/document_symbols_runner.go:238 noopProjectClient.WatchFiles 887ebb05279d466d
    fn watch_files(
        &self,
        _ctx: &Context,
        _id: project::WatcherID,
        _watchers: Vec<lsproto::FileSystemWatcher>,
    ) -> Result<(), lsproto::Error> {
        Ok(())
    }

    // go test: internal/effecttest/document_symbols_runner.go:242 noopProjectClient.UnwatchFiles 30216a83c229aca0
    fn unwatch_files(&self, _ctx: &Context, _id: project::WatcherID) -> Result<(), lsproto::Error> {
        Ok(())
    }

    // go test: internal/effecttest/document_symbols_runner.go:244 noopProjectClient.RegisterContentMapperExtensions 62f089b106e57603
    fn register_content_mapper_extensions(
        &self,
        _ctx: &Context,
        _extensions: Vec<String>,
    ) -> Result<(), lsproto::Error> {
        Ok(())
    }

    // go test: internal/effecttest/document_symbols_runner.go:248 noopProjectClient.RefreshDiagnostics 1fa72b9a191c0d7e
    fn refresh_diagnostics(&self, _ctx: &Context) -> Result<(), lsproto::Error> {
        Ok(())
    }

    // go test: internal/effecttest/document_symbols_runner.go:250 noopProjectClient.PublishDiagnostics ab1d1535f82ebe39
    fn publish_diagnostics(
        &self,
        _ctx: &Context,
        _params: lsproto::PublishDiagnosticsParams,
    ) -> Result<(), lsproto::Error> {
        Ok(())
    }

    // go test: internal/effecttest/document_symbols_runner.go:254 noopProjectClient.RefreshInlayHints 668cd929ff2e3caa
    fn refresh_inlay_hints(&self, _ctx: &Context) -> Result<(), lsproto::Error> {
        Ok(())
    }

    // go test: internal/effecttest/document_symbols_runner.go:256 noopProjectClient.RefreshCodeLens 5d473f1f00b00385
    fn refresh_code_lens(&self, _ctx: &Context) -> Result<(), lsproto::Error> {
        Ok(())
    }

    // go test: internal/effecttest/document_symbols_runner.go:258 noopProjectClient.ProgressStart bcadecd239fe9cf5
    fn progress_start(&self, _message: &'static Message, _args: &[&dyn Display]) {}

    // go test: internal/effecttest/document_symbols_runner.go:260 noopProjectClient.ProgressFinish 47aea43ac0426a21
    fn progress_finish(&self, _message: &'static Message, _args: &[&dyn Display]) {}

    // go test: internal/effecttest/document_symbols_runner.go:262 noopProjectClient.SendTelemetry a764820ff2c95563
    fn send_telemetry(
        &self,
        _ctx: &Context,
        _telemetry: lsproto::TelemetryEvent,
    ) -> Result<(), lsproto::Error> {
        Ok(())
    }

    // go test: internal/effecttest/document_symbols_runner.go:264 noopProjectClient.IsActive 22c40ce91f45c1ed
    fn is_active(&self) -> bool {
        true
    }

    // go test: internal/effecttest/document_symbols_runner.go:266 noopProjectClient.SetLocale 1cdc254d51b8948c
    fn set_locale(&self, _locale: &str) {}

    // go test: internal/effecttest/document_symbols_runner.go:268 noopProjectClient.GetLocale 7e39d049afb16543
    fn get_locale(&self) -> Locale {
        Locale::DEFAULT
    }
}
