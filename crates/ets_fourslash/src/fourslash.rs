//! Rust-only: port of the part of typescript-go's `fourslash/fourslash.go` the Effect runners use, plus the Effect
//! additions of `_patches/typescript/010-fourslash-fourslash.patch` (`PrepareTestFSCallback`, the quick-fix and
//! refactor helpers). As in Go, a `FourslashTest` drives a real language server (tsrs's `tsrs_lsp::Server`) through
//! tsrs's in-process LSP client (`tsrs_lsp::lsptestutil`, the port of Go's `testutil/lsptestutil`): JSON-RPC
//! messages over in-memory pipes, so every request takes the editor's path through the server.
//!
//! Left out (no Effect runner uses them): state and request baselines (`baselineState`,
//! `baselineRequestOrNotification`, `verifyBaselines`), markers' and ranges' `GoTo*` helpers beyond `GoToBOF`,
//! `GoToMarker` and `GoToFile`, the `verify*` assertions other than the quick-info, error-count, diagnostics,
//! import-fix and completion code-action ones, global compiler options from `// @option:` lines
//! (`SetOptionsFromTestConfig`; a case with a tsconfig may not have any, and a case without one gets the default
//! options), symlinks, `@tsc` fixtures and content mappers.

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use ets_go::testing::T;
use ets_shim::core::json::Value;
use ets_shim::core::tspath;
use ets_shim::core::{
    CompilerOptions, JsxEmit, P, ScriptTarget, TextChange, TextPos, TextRange, Tristate,
};
use ets_shim::ls::lsconv::{self, Converters, LSPLineMap};
use ets_shim::ls::lsutil::{self, UserPreferences};
use ets_shim::lsp::lsptestutil::{self, CloseClient, LSPClient};
use ets_shim::lsp::{Reader, ServerOptions, Writer};
use ets_shim::lsproto::{self, Json};
use ets_shim::project;
use ets_shim::vfs::vfstest::MapFile;
use ets_shim::vfs::{FS, bundled, vfstest};
use parking_lot::{Mutex, RwLock};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::*;

/// Rust-only: one entry of the test file system map `NewFourslash` builds.
///
/// Go's map is a `map[string]any`, whose values are the test files' contents as `string`s, `vfstest.Symlink`s,
/// and whatever a `PrepareTestFSCallback` adds, such as the `*fstest.MapFile`s of the mounted Effect packages.
#[derive(Clone, Debug)]
pub enum TestFSEntry {
    String(String),
    MapFile(MapFile),
}

/// Rust-only: Go's `map[string]any` test file system of `NewFourslash`.
pub type TestFS = FxHashMap<String, TestFSEntry>;

/// Rust-only: `_patches/typescript/010-fourslash-fourslash.patch` `PrepareTestFSCallback`.
// PrepareTestFSCallback is invoked before the VFS is created in NewFourslash.
// It allows external packages to modify the test filesystem (e.g., mount real node_modules).
pub type PrepareTestFSCallback = fn(testfs: &mut TestFS);

static PREPARE_TEST_FS_CALLBACK: RwLock<Option<PrepareTestFSCallback>> = RwLock::new(None);

/// Rust-only: `_patches/typescript/010-fourslash-fourslash.patch` `RegisterPrepareTestFSCallback`.
// RegisterPrepareTestFSCallback registers a callback to be called before the VFS
// is created from the test filesystem map. The callback can add or modify entries.
pub fn register_prepare_test_fs_callback(cb: PrepareTestFSCallback) {
    *PREPARE_TEST_FS_CALLBACK.write() = Some(cb);
}

/// Rust-only: `fourslash.go` `FourslashTest`.
///
/// Go's `scriptInfos` map is shared with the converters' line-map lookup (Go's closure reads the map), so it sits
/// behind a lock; so are the user preferences the server-request handler answers `workspace/configuration` with.
pub struct FourslashTest {
    client: Arc<LSPClient>,
    close_client: Option<CloseClient>,
    vfs: Arc<dyn FS>,

    test_data: TestData,
    open_files: FxHashSet<String>,

    script_infos: Arc<Mutex<FxHashMap<String, ScriptInfo>>>,
    converters: Arc<Converters>,

    user_preferences: Arc<Mutex<UserPreferences>>,
    current_caret_position: lsproto::Position,
    last_known_marker_name: Option<String>,
    active_filename: String,
    selection_end: Option<lsproto::Position>,

    capabilities: lsproto::ClientCapabilities,
}

/// Rust-only: `fourslash.go` `scriptInfo`.
#[derive(Clone)]
struct ScriptInfo {
    file_name: String,
    content: String,
    line_map: Arc<LSPLineMap>,
    version: i32,
}

/// Rust-only: `fourslash.go` `newScriptInfoFromFileName`.
fn new_script_info_from_file_name(file_name: &str, content: &str) -> ScriptInfo {
    ScriptInfo {
        file_name: file_name.to_string(),
        content: content.to_string(),
        line_map: lsconv::compute_lsp_line_starts(content),
        version: 1,
    }
}

impl ScriptInfo {
    /// Rust-only: `fourslash.go` `scriptInfo.editContent`.
    fn edit_content(&mut self, change: &TextChange) {
        self.content = change.apply_to(&self.content);
        self.line_map = lsconv::compute_lsp_line_starts(&self.content);
        self.version += 1;
    }
}

impl lsconv::Script for ScriptInfo {
    fn file_name(&self) -> &str {
        &self.file_name
    }

    fn original_file_name(&self) -> &str {
        &self.file_name
    }

    fn text(&self) -> &str {
        &self.content
    }

    fn span_map(&self) -> Option<&ets_shim::ls::spanmap::SpanMap> {
        None
    }

    fn original_text(&self) -> &str {
        &self.content
    }
}

/// Rust-only: `fourslash.go` `testConverters.PositionToLineAndCharacter`.
pub(crate) fn position_to_line_and_character(
    converters: &Converters,
    script: &dyn lsconv::Script,
    position: TextPos,
) -> lsproto::Position {
    let (lsp_position, _) = converters.to_lsp_position(script, position);
    lsp_position
}

/// Rust-only: `fourslash.go` `testConverters.LineAndCharacterToPosition`.
fn line_and_character_to_position<S: lsconv::Script + Clone>(
    converters: &Converters,
    script: &S,
    position: lsproto::Position,
) -> u32 {
    let positions =
        converters.from_lsp_position(script.clone(), position, ets_shim::ls::spanmap::Feature::All);
    assert!(positions.len() == 1, "fourslash script must have exactly one position projection");
    positions[0].position
}

/// Rust-only: `fourslash.go` `rootDir`.
pub const ROOT_DIR: &str = "/";

/// Rust-only: `fourslash.go` `parseCache`, shared by every fourslash test of the process.
static PARSE_CACHE: LazyLock<Arc<project::ParseCache>> = LazyLock::new(|| {
    Arc::new(project::new_parse_cache(project::RefCountCacheOptions { disable_deletion: true }))
});

/// Rust-only: the placeholder streams of `ServerOptions` (`lsptestutil::new_lsp_client` replaces them with its pipes;
/// Go leaves `In` and `Out` nil).
struct NoStream;

impl Reader for NoStream {
    fn read(&mut self) -> (Option<lsproto::Message>, Option<lsproto::Error>) {
        (None, Some(lsproto::Error::tagged(lsproto::ErrorTag::EOF, "EOF")))
    }
}

impl Writer for NoStream {
    fn write(&mut self, _msg: &lsproto::Message) -> Result<(), lsproto::Error> {
        Ok(())
    }
}

/// Rust-only: `fourslash.go` `NewFourslash`. Go returns the test and a `done` function; here `done` is
/// `FourslashTest::done` (also run on drop).
pub fn new_fourslash(
    t: &T,
    capabilities: Option<lsproto::ClientCapabilities>,
    content: &str,
) -> FourslashTest {
    // fourslash.go `newFourslash`
    let file_name = get_base_file_name_from_test(t) + tspath::EXTENSION_TS;
    let mut testfs: TestFS = TestFS::default();
    let mut script_infos: FxHashMap<String, ScriptInfo> = FxHashMap::default();
    let test_data = parse_test_data(content, &file_name);
    for file in &test_data.files {
        let file_path = &file.file_name;
        // Dynamic files (e.g., untitled:) shouldn't be added to the VFS
        if !tspath::is_dynamic_file_name(file_path) {
            testfs.insert(file_path.clone(), TestFSEntry::String(file.content.clone()));
        }
        script_infos
            .insert(file_path.clone(), new_script_info_from_file_name(file_path, &file.content));
    }

    // Rust-only: symlinks (`vfstest.Symlink`) are not supported by the stand-in.
    assert!(test_data.symlinks.is_empty(), "the Rust fourslash stand-in does not support symlinks");

    // EFFECT HOOK: Allow external packages to modify test filesystem before VFS creation
    let prepare_test_fs_callback = *PREPARE_TEST_FS_CALLBACK.read();
    if let Some(prepare_test_fs_callback) = prepare_test_fs_callback {
        prepare_test_fs_callback(&mut testfs);
    }

    // !!! use default compiler options for inferred project as base
    let compiler_options = CompilerOptions {
        skip_default_lib_check: Tristate::True,
        target: ScriptTarget::LATEST_STANDARD,
        jsx: JsxEmit::PRESERVE,
        ..CompilerOptions::default()
    };
    // Rust-only: Go's `harnessutil.SetOptionsFromTestConfig` applies the global `// @option:` lines; a case with a
    // tsconfig may not have any (`ParseTestData` fails), and the Effect runners always have one.
    assert!(
        test_data.global_options.is_empty(),
        "the Rust fourslash stand-in does not support global options: {:?}",
        test_data.global_options
    );

    let fs_from_map = vfstest::from_map(
        testfs.into_iter().map(|(path, entry)| match entry {
            TestFSEntry::String(content) => (path, MapFile::from(content)),
            TestFSEntry::MapFile(file) => (path, file),
        }),
        true,
    );
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(fs_from_map));

    let server_opts = ServerOptions {
        in_: Box::new(NoStream),
        out: Box::new(NoStream),
        err: Box::new(std::io::sink()),

        cwd: ROOT_DIR.to_string(),
        fs: Some(Arc::clone(&fs)),
        default_library_path: bundled::lib_path(),
        typings_location: String::new(),

        parse_cache: Some(Arc::clone(&PARSE_CACHE)),
        npm_install: None,
        progress_delay: Duration::ZERO,
        set_parent_process_id: None,
    };

    let script_infos = Arc::new(Mutex::new(script_infos));
    let converters = {
        let script_infos = Arc::clone(&script_infos);
        lsconv::new_converters(lsproto::PositionEncodingKind::UTF8, move |file_name: &str| {
            script_infos.lock().get(file_name).map(|script_info| Arc::clone(&script_info.line_map))
        })
    };

    let user_preferences = Arc::new(Mutex::new(lsutil::new_default_user_preferences()));
    let handler = {
        let user_preferences = Arc::clone(&user_preferences);
        Box::new(move |req: &lsproto::RequestMessage| {
            handle_server_request(&user_preferences.lock(), req)
        })
    };
    let (client, close_client) = lsptestutil::new_lsp_client(server_opts, Some(handler));

    let mut f = FourslashTest {
        client,
        close_client: Some(close_client),
        vfs: fs,
        test_data,
        open_files: FxHashSet::default(),
        script_infos,
        converters,
        user_preferences,
        current_caret_position: lsproto::Position::default(),
        last_known_marker_name: None,
        active_filename: String::new(),
        selection_end: None,
        capabilities: lsproto::ClientCapabilities::default(),
    };

    // !!! temporary; remove when we have `handleDidChangeConfiguration`/implicit project config support
    // !!! replace with a proper request *after initialize*
    f.client.set_compiler_options_for_inferred_projects(Some(P::new(compiler_options)));
    f.initialize(t, capabilities);

    assert!(
        !f.test_data.is_state_baselining_enabled(),
        "the Rust fourslash stand-in does not support state baselines"
    );
    let open: Vec<String> = f
        .test_data
        .files
        .iter()
        .filter(|file| file.open)
        .map(|file| file.file_name.clone())
        .collect();
    for file_name in open {
        f.open_file(t, &file_name);
    }
    f.active_filename = f.test_data.files[0].file_name.clone();

    f
}

/// Rust-only: `fourslash.go` `FourslashTest.handleServerRequest`. A request it does not handle gets `None`, which
/// `lsptestutil` answers with Go's `Unknown method` error.
fn handle_server_request(
    user_preferences: &UserPreferences,
    req: &lsproto::RequestMessage,
) -> Option<lsproto::ResponseMessage> {
    match req.method {
        lsproto::Method::WORKSPACE_CONFIGURATION => {
            // Return current user preferences for each requested section.
            // The server requests multiple sections (js/ts, typescript, javascript, editor);
            // we return user preferences for "js/ts" and nil for others.
            let params = req.unmarshal_params::<lsproto::ConfigurationParams>();
            let Ok(params) = params else {
                return Some(lsproto::ResponseMessage {
                    id: req.id.clone(),
                    result: Some(Value::Array(vec![user_preferences.marshal_json_to()])),
                    ..Default::default()
                });
            };
            let results = params
                .items
                .iter()
                .map(|item| {
                    if item.section.as_deref() == Some("js/ts") {
                        user_preferences.marshal_json_to()
                    } else {
                        Value::Null
                    }
                })
                .collect();
            Some(lsproto::ResponseMessage {
                id: req.id.clone(),
                result: Some(Value::Array(results)),
                ..Default::default()
            })
        }
        // Accept all capability registrations
        // Accept all capability unregistrations
        lsproto::Method::CLIENT_REGISTER_CAPABILITY
        | lsproto::Method::CLIENT_UNREGISTER_CAPABILITY => Some(lsproto::ResponseMessage {
            id: req.id.clone(),
            result: Some(lsproto::Null.to_json()),
            ..Default::default()
        }),
        // Unknown server request
        _ => None,
    }
}

/// Rust-only: `fourslash.go` `getBaseFileNameFromTest`.
fn get_base_file_name_from_test(t: &T) -> String {
    let name = t.name();
    let name = name.rsplit('/').next().unwrap_or_default();
    let name = name.strip_prefix("Test").unwrap_or(name);
    ets_shim::core::stringutil::lower_first_char(name)
}

/// Rust-only: `fourslash.go` `isSuggestionDiagnostic`.
fn is_suggestion_diagnostic(diag: &lsproto::Diagnostic) -> bool {
    diag.severity == Some(lsproto::DiagnosticSeverity::HINT)
}

// Rust-only: fourslash.go showCodeLensLocationsCommandName.
const SHOW_CODE_LENS_LOCATIONS_COMMAND_NAME: &str = "typescript.showCodeLensLocations";

impl FourslashTest {
    /// Rust-only: `fourslash.go` `FourslashTest.initialize`.
    fn initialize(&mut self, t: &T, capabilities: Option<lsproto::ClientCapabilities>) {
        let initialization_options = lsproto::InitializationOptions {
            code_lens_show_locations_command_name: Some(
                SHOW_CODE_LENS_LOCATIONS_COMMAND_NAME.to_string(),
            ),
            ..Default::default()
        };
        let mut params = lsproto::InitializeParams {
            locale: Some("en-US".to_string()),
            initialization_options: Some(lsproto::InitializationOptionsOrNull {
                initialization_options: Some(initialization_options),
            }),
            ..Default::default()
        };
        params.capabilities = get_capabilities_with_defaults(capabilities);
        self.capabilities = params.capabilities.clone();
        let (resp, result) = self.client.send_request(lsproto::INITIALIZE_INFO, params);
        if result.is_none() && resp.error.is_none() {
            t.fatal("Initialize request failed");
        }
        if let Some(error) = &resp.error {
            t.fatal(format!("Initialize request returned error: {}", error.message));
        }
        self.client
            .send_notification(lsproto::INITIALIZED_INFO, lsproto::InitializedParams::default());

        // Wait for the initial configuration exchange to complete
        // The server will send workspace/configuration as part of handleInitialized
        self.client.server.init_complete().wait();
    }

    /// Rust-only: Go's `done` function returned by `NewFourslash`: closes the client (its error fails the test);
    /// `verifyBaselines` has nothing to verify (no baseline commands are ported).
    pub fn done(&mut self, t: &T) {
        if let Some(close_client) = self.close_client.take() {
            if let Err(err) = close_client.close() {
                t.error(format!("goroutine error: {}", err.message));
            }
        }
    }

    /// Rust-only: the language server's session (for runners that need to reach the projects behind it).
    pub fn session(&self) -> &Arc<project::Session> {
        self.client.server.session()
    }

    /// Rust-only: the shim accessor `fourslash.FourslashTest_client` (`_tools/gen_shims` extra-shim).
    pub fn client(&self) -> &LSPClient {
        &self.client
    }

    /// Rust-only: `fourslash.go` `FourslashTest.sendRequest` (with `baselineProjects`; there are no baselines).
    pub fn send_request<Params: Json, Resp: Json>(
        &self,
        t: &T,
        info: lsproto::RequestInfo<Params, Resp>,
        params: Params,
    ) -> Resp {
        let (res_msg, result) = self.client.send_request(info, params);
        if let Some(error) = &res_msg.error {
            t.fatal(format!("{} request returned error: {}", info.method.0, error.message));
        }
        let Some(result) = result else {
            t.fatal(format!("Unexpected {} response type", info.method.0));
        };
        result
    }

    /// Rust-only: `fourslash.go` `FourslashTest.sendNotification` (no baselines; `updateState` tracks open files).
    fn send_notification<Params: Json>(
        &mut self,
        info: lsproto::NotificationInfo<Params>,
        params: Params,
        opened: Option<&str>,
    ) {
        if let Some(opened) = opened {
            self.open_files.insert(opened.to_string());
        }
        self.client.send_notification(info, params);
    }

    /// Rust-only: `fourslash.go` `FourslashTest.GetOptions`.
    pub fn get_options(&self) -> UserPreferences {
        self.user_preferences.lock().clone()
    }

    /// Rust-only: `fourslash.go` `FourslashTest.Configure`.
    pub fn configure(&mut self, _t: &T, config: UserPreferences) {
        // We send 'js/ts' by default because that is what we expect the primary config to be in vscode and VS (one
        // set of preferences for both languages). This should be fine in fourslash since tests that need
        // multiple options usually send reconfiguration commands for each `verify` anyways
        let settings = config.marshal_json_to();
        *self.user_preferences.lock() = config;
        let mut object = ets_shim::core::json::JsonMap::default();
        object.insert("js/ts".to_string(), settings);
        self.send_notification(
            lsproto::WORKSPACE_DID_CHANGE_CONFIGURATION_INFO,
            lsproto::DidChangeConfigurationParams { settings: Value::Object(object) },
            None,
        );
    }

    /// Rust-only: `fourslash.go` `FourslashTest.ConfigureWithReset`. Go returns the reset function; here it is the
    /// returned preferences, passed back to `configure`.
    pub fn configure_with_reset(&mut self, t: &T, config: UserPreferences) -> UserPreferences {
        let original_config = self.get_options();
        self.configure(t, config);
        original_config
    }

    /// Rust-only: `fourslash.go` `FourslashTest.GoToBOF`.
    pub fn go_to_bof(&mut self, t: &T) {
        self.go_to_position(t, lsproto::Position { line: 0, character: 0 });
    }

    /// Rust-only: `fourslash.go` `FourslashTest.GoToMarker`.
    pub fn go_to_marker(&mut self, t: &T, marker_name: &str) {
        let Some(&marker) = self.test_data.marker_positions.get(marker_name) else {
            t.fatal(format!("Marker '{marker_name}' not found"));
        };
        let marker = self.test_data.markers[marker].clone();
        // fourslash.go `FourslashTest.goToMarker`
        self.ensure_active_file(t, &marker.file_name);
        self.go_to_position(t, marker.ls_position);
        self.last_known_marker_name = marker.name;
    }

    /// Rust-only: `fourslash.go` `FourslashTest.ensureActiveFile`.
    fn ensure_active_file(&mut self, t: &T, filename: &str) {
        if self.active_filename != filename {
            if self.open_files.contains(filename) {
                self.active_filename = filename.to_string();
            } else {
                self.open_file(t, filename);
            }
        }
    }

    /// Rust-only: `fourslash.go` `FourslashTest.getCurrentPositionPrefix`.
    pub fn get_current_position_prefix(&self) -> String {
        if let Some(name) = &self.last_known_marker_name {
            return format!("At marker '{name}': ");
        }
        format!(
            "At position {}(Ln {}, Col {}): ",
            self.active_filename,
            self.current_caret_position.line,
            self.current_caret_position.character
        )
    }

    /// Rust-only: `fourslash.go` `FourslashTest.VerifyQuickInfoAt`.
    pub fn verify_quick_info_at(
        &mut self,
        t: &T,
        marker: &str,
        expected_text: &str,
        expected_documentation: &str,
    ) {
        self.go_to_marker(t, marker);
        let Some(hover) = self.get_quick_info_at_current_position(t) else {
            t.fatal(format!(
                "Expected hover result at marker '{}' but got nil",
                self.last_known_marker_name.as_deref().unwrap_or_default()
            ));
        };
        verify_hover_content(
            t,
            &hover.contents,
            expected_text,
            expected_documentation,
            &self.get_current_position_prefix(),
        );
    }

    /// Rust-only: `fourslash.go` `FourslashTest.getQuickInfoAtCurrentPosition`.
    pub fn get_quick_info_at_current_position(&self, t: &T) -> Option<lsproto::Hover> {
        let params = lsproto::HoverParams {
            text_document: lsproto::TextDocumentIdentifier {
                uri: lsconv::file_name_to_document_uri(&self.active_filename),
            },
            position: self.current_caret_position,
            ..Default::default()
        };
        let result = self.send_request(t, lsproto::TEXT_DOCUMENT_HOVER_INFO, params);
        result.hover
    }

    /// Rust-only: `fourslash.go` `FourslashTest.GetCompletions` (with `getCompletions` inlined).
    pub fn get_completions(
        &mut self,
        t: &T,
        user_preferences: Option<&UserPreferences>,
    ) -> Option<lsproto::CompletionList> {
        let params = lsproto::CompletionParams {
            text_document: lsproto::TextDocumentIdentifier {
                uri: lsconv::file_name_to_document_uri(&self.active_filename),
            },
            position: self.current_caret_position,
            context: Some(lsproto::CompletionContext::default()),
            ..Default::default()
        };
        // Go's `defer reset()`: the original preferences are restored after the request.
        let mut reset: Option<UserPreferences> = None;
        if let Some(user_preferences) = user_preferences {
            let mut preferences = user_preferences.clone();
            preferences.format_code_settings =
                self.user_preferences.lock().format_code_settings.clone();
            reset = Some(self.configure_with_reset(t, preferences));
        }
        let mut result = self.send_request(t, lsproto::TEXT_DOCUMENT_COMPLETION_INFO, params);
        if let Some(original) = reset {
            self.configure(t, original);
        }
        // For performance, the server may return unsorted completion lists.
        // The client is expected to sort them by SortText and then by Label.
        // We are the client here.
        if let Some(list) = &mut result.list {
            list.items.sort_by(ets_shim::ls::compare_completion_entries);
        }
        result.list
    }

    /// Rust-only: `fourslash.go` `FourslashTest.Replace` (with `replaceWorker` inlined; no state baseline).
    pub fn replace(&mut self, t: &T, start: i64, length: i64, text: &str) {
        let active_filename = self.active_filename.clone();
        self.edit_script_and_update_markers(t, &active_filename, start, start + length, text);
        // f.checkPostEditInvariants() // !!! do we need this?
    }

    /// Rust-only: `fourslash.go` `FourslashTest.getDiagnostics`.
    fn get_diagnostics(&self, t: &T, file_name: &str) -> Vec<lsproto::Diagnostic> {
        let params = lsproto::DocumentDiagnosticParams {
            text_document: lsproto::TextDocumentIdentifier {
                uri: lsconv::file_name_to_document_uri(file_name),
            },
            ..Default::default()
        };
        let result = self.send_request(t, lsproto::TEXT_DOCUMENT_DIAGNOSTIC_INFO, params);
        if let Some(report) = result.full_document_diagnostic_report {
            return report.items;
        }
        Vec::new()
    }

    /// Rust-only: `fourslash.go` `FourslashTest.VerifyNumberOfErrorsInCurrentFile`.
    // VerifyNumberOfErrorsInCurrentFile verifies that the current file has the expected number of errors.
    pub fn verify_number_of_errors_in_current_file(&self, t: &T, expected_count: usize) {
        let diagnostics = self.get_diagnostics(t, &self.active_filename);
        // Filter to only include errors (not suggestions/hints)
        let errors: Vec<_> = diagnostics.iter().filter(|d| !is_suggestion_diagnostic(d)).collect();
        if errors.len() != expected_count {
            t.fatal(format!(
                "Expected {expected_count} errors in current file, but got {}",
                errors.len()
            ));
        }
    }

    /// Rust-only: `fourslash.go` `FourslashTest.VerifyNoErrors`.
    // VerifyNoErrors verifies that no errors exist in any open files.
    pub fn verify_no_errors(&self, t: &T) {
        for file_name in &self.open_files {
            let diagnostics = self.get_diagnostics(t, file_name);
            // Filter to only include errors (not suggestions/hints)
            let errors: Vec<_> =
                diagnostics.iter().filter(|d| !is_suggestion_diagnostic(d)).collect();
            if !errors.is_empty() {
                let messages: Vec<String> =
                    errors.iter().map(|err| err.message.as_string()).collect();
                // Go's `%v` of a `[]string`.
                t.fatal(format!(
                    "Expected no errors but found {} in {file_name}: [{}]",
                    errors.len(),
                    messages.join(" ")
                ));
            }
        }
    }

    /// Rust-only: `fourslash.go` `FourslashTest.goToPosition`.
    fn go_to_position(&mut self, _t: &T, position: lsproto::Position) {
        self.current_caret_position = position;
        self.selection_end = None;
    }

    /// Rust-only: `fourslash.go` `FourslashTest.GoToFile`.
    pub fn go_to_file(&mut self, t: &T, filename: &str) {
        self.open_file(t, &to_rooted_file_path(filename));
    }

    /// Rust-only: `fourslash.go` `FourslashTest.openFile`.
    fn open_file(&mut self, t: &T, filename: &str) {
        let script = self.get_script_info(filename);
        let script = match script {
            Some(script) => script,
            None => match self.vfs.read_file(filename) {
                Some(content) => {
                    let script = new_script_info_from_file_name(filename, &content);
                    self.script_infos.lock().insert(filename.to_string(), script.clone());
                    script
                }
                None => t.fatal(format!("File {filename} not found in test data")),
            },
        };
        self.active_filename = filename.to_string();
        self.send_notification(
            lsproto::TEXT_DOCUMENT_DID_OPEN_INFO,
            lsproto::DidOpenTextDocumentParams {
                text_document: lsproto::TextDocumentItem {
                    uri: lsconv::file_name_to_document_uri(filename),
                    language_id: get_language_kind(filename),
                    text: script.content,
                    ..Default::default()
                },
            },
            Some(filename),
        );
    }

    /// Rust-only: `fourslash.go` `FourslashTest.applyTextEdits`. Updates the current caret position.
    fn apply_text_edits(&mut self, t: &T, edits: &mut [lsproto::TextEdit]) -> i64 {
        let script = self.get_script_info(&self.active_filename).unwrap();
        ets_go::slices::sort_func(edits, |a, b| {
            let a_start = line_and_character_to_position(&self.converters, &script, a.range.start);
            let b_start = line_and_character_to_position(&self.converters, &script, b.range.start);
            i64::from(a_start).cmp(&i64::from(b_start))
        });

        let mut total_offset: i64 = 0;
        let mut current_caret_position = i64::from(line_and_character_to_position(
            &self.converters,
            &script,
            self.current_caret_position,
        ));
        // Apply edits in reverse order to avoid affecting the positions of earlier edits.
        for edit in edits.iter().rev() {
            // Rust-only: Go's `script` is a pointer to the entry, so it sees the edits applied so far; read it again.
            let script = self.get_script_info(&self.active_filename).unwrap();
            let start = i64::from(line_and_character_to_position(
                &self.converters,
                &script,
                edit.range.start,
            ));
            let end = i64::from(line_and_character_to_position(
                &self.converters,
                &script,
                edit.range.end,
            ));
            let active_filename = self.active_filename.clone();
            self.edit_script_and_update_markers(t, &active_filename, start, end, &edit.new_text);

            let delta = len(&edit.new_text) - (end - start);
            if start <= current_caret_position {
                if end <= current_caret_position {
                    // The entirety of the edit span falls before the caret position, shift the caret accordingly
                    current_caret_position += delta;
                } else {
                    // The span being replaced includes the caret position, place the caret at the beginning of the span
                    current_caret_position = start;
                }
            }
            total_offset += delta;
        }
        // Rust-only: Go converts with the script it read before the edits (`script` is a pointer, so it sees them);
        // read it again.
        let script = self.get_script_info(&self.active_filename).unwrap();
        self.current_caret_position = position_to_line_and_character(
            &self.converters,
            &script,
            TextPos::from(u32::try_from(current_caret_position).unwrap()),
        );
        total_offset
    }

    /// Rust-only: `fourslash.go` `FourslashTest.editScriptAndUpdateMarkers`.
    // Edits the script and updates marker and range positions accordingly.
    // This does not update the current caret position.
    fn edit_script_and_update_markers(
        &mut self,
        t: &T,
        file_name: &str,
        edit_start: i64,
        edit_end: i64,
        new_text: &str,
    ) {
        self.edit_script_and_update_markers_worker(
            t,
            file_name,
            vec![TextChange {
                text_range: TextRange::new(to_u32(edit_start), to_u32(edit_end)),
                new_text: new_text.to_string(),
            }],
        );
    }

    /// Rust-only: `fourslash.go` `FourslashTest.editScriptAndUpdateMarkersWorker`.
    fn edit_script_and_update_markers_worker(
        &mut self,
        t: &T,
        file_name: &str,
        changes: Vec<TextChange>,
    ) {
        // Sort changes by position (ascending) so we can apply in reverse
        let mut sorted_changes = changes;
        ets_go::slices::sort_func(&mut sorted_changes, |a, b| {
            text_pos(a.text_range.pos()).cmp(&text_pos(b.text_range.pos()))
        });

        // Apply changes in reverse order to preserve positions of earlier changes
        for change in sorted_changes.iter().rev() {
            let edit_start = i64::from(text_pos(change.text_range.pos()));
            let edit_end = i64::from(text_pos(change.text_range.end()));
            let script = self.edit_script(t, file_name, change);
            for marker in &mut self.test_data.markers {
                if marker.file_name == file_name {
                    marker.position =
                        update_position(marker.position, edit_start, edit_end, &change.new_text);
                    marker.ls_position = position_to_line_and_character(
                        &self.converters,
                        &script,
                        TextPos::from(to_u32(marker.position)),
                    );
                }
            }
            for range_marker in &mut self.test_data.ranges {
                if range_marker.file_name == file_name {
                    let start = update_position(
                        i64::from(text_pos(range_marker.range.pos())),
                        edit_start,
                        edit_end,
                        &change.new_text,
                    );
                    let end = update_position(
                        i64::from(text_pos(range_marker.range.end())),
                        edit_start,
                        edit_end,
                        &change.new_text,
                    );
                    range_marker.range = TextRange::new(to_u32(start), to_u32(end));
                    (range_marker.ls_range, _) =
                        self.converters.to_lsp_range(&script, range_marker.range);
                }
            }
        }
    }

    /// Rust-only: `fourslash.go` `FourslashTest.editScript`.
    fn edit_script(&mut self, t: &T, file_name: &str, change: &TextChange) -> ScriptInfo {
        let Some(mut script) = self.get_or_load_script_info(file_name) else {
            panic!("Script info for file {file_name} not found");
        };
        let (change_range, _) = self.converters.to_lsp_range(&script, change.text_range);
        script.edit_content(change);
        self.script_infos.lock().insert(file_name.to_string(), script.clone());
        if let Err(err) = self.vfs.write_file(file_name, &script.content) {
            t.fatal(format!("failed to write to VFS for {file_name}: {err}"));
        }
        self.send_notification(
            lsproto::TEXT_DOCUMENT_DID_CHANGE_INFO,
            lsproto::DidChangeTextDocumentParams {
                text_document: lsproto::VersionedTextDocumentIdentifier {
                    uri: lsconv::file_name_to_document_uri(file_name),
                    version: script.version,
                },
                content_changes: vec![lsproto::TextDocumentContentChangePartialOrWholeDocument {
                    partial: Some(lsproto::TextDocumentContentChangePartial {
                        range: change_range,
                        text: change.new_text.clone(),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
            },
            None,
        );
        script
    }

    /// Rust-only: `fourslash.go` `FourslashTest.getScriptInfo` (a copy of the entry: Go hands out the pointer).
    fn get_script_info(&self, file_name: &str) -> Option<ScriptInfo> {
        self.script_infos.lock().get(file_name).cloned()
    }

    /// Rust-only: `fourslash.go` `FourslashTest.getOrLoadScriptInfo`.
    fn get_or_load_script_info(&self, file_name: &str) -> Option<ScriptInfo> {
        if let Some(script) = self.get_script_info(file_name) {
            return Some(script);
        }
        if let Some(content) = self.vfs.read_file(file_name) {
            let script = new_script_info_from_file_name(file_name, &content);
            self.script_infos.lock().insert(file_name.to_string(), script.clone());
            return Some(script);
        }
        None
    }
}

/// Rust-only: the markers, ranges, diagnostics and import-fix helpers of `fourslash.go` the Effect tests use.
impl FourslashTest {
    /// Rust-only: `fourslash.go` `FourslashTest.MarkerByName` (`None` for an unknown name: Go's nil).
    pub fn marker_by_name(&self, _t: &T, name: &str) -> Option<&Marker> {
        self.test_data.marker_positions.get(name).map(|&marker| &self.test_data.markers[marker])
    }

    /// Rust-only: `fourslash.go` `FourslashTest.Ranges`.
    pub fn ranges(&self) -> &[RangeMarker] {
        &self.test_data.ranges
    }

    /// Rust-only: `fourslash.go` `FourslashTest.getRangesInFile`.
    fn get_ranges_in_file(&self, file_name: &str) -> Vec<RangeMarker> {
        let mut ranges_in_file = Vec::new();
        for range_marker in &self.test_data.ranges {
            if range_marker.file_name() == file_name {
                ranges_in_file.push(range_marker.clone());
            }
        }
        ranges_in_file
    }

    /// Rust-only: `fourslash.go` `FourslashTest.VerifyImportFixAtPosition`. Go's `preferences` pointer is an
    /// `Option`.
    pub fn verify_import_fix_at_position(
        &mut self,
        t: &T,
        expected_texts: &[&str],
        preferences: Option<&UserPreferences>,
    ) {
        let file_name = self.active_filename.clone();
        let ranges = self.ranges();
        let mut filtered_ranges: Vec<RangeMarker> = Vec::new();
        for r in ranges {
            if r.file_name() == file_name {
                filtered_ranges.push(r.clone());
            }
        }
        if filtered_ranges.len() > 1 {
            t.fatal("Exactly one range should be specified in the testfile.");
        }
        let range_marker = filtered_ranges.into_iter().next();

        // Rust-only: Go's `defer reset()`: the original preferences are restored on every return below.
        let reset =
            preferences.map(|preferences| self.configure_with_reset(t, preferences.clone()));
        self.verify_import_fix_at_position_worker(t, expected_texts, range_marker.as_ref());
        if let Some(reset) = reset {
            self.configure(t, reset);
        }
    }

    /// Rust-only: the body of `VerifyImportFixAtPosition` after its preferences are configured.
    fn verify_import_fix_at_position_worker(
        &mut self,
        t: &T,
        expected_texts: &[&str],
        range_marker: Option<&RangeMarker>,
    ) {
        // Get diagnostics at the current position to find errors that need import fixes
        let diag_params = lsproto::DocumentDiagnosticParams {
            text_document: lsproto::TextDocumentIdentifier {
                uri: lsconv::file_name_to_document_uri(&self.active_filename),
            },
            ..Default::default()
        };
        let diag_result = self.send_request(t, lsproto::TEXT_DOCUMENT_DIAGNOSTIC_INFO, diag_params);

        let mut diagnostics: Vec<lsproto::Diagnostic> = Vec::new();
        if let Some(report) = diag_result.full_document_diagnostic_report {
            diagnostics = report.items;
        }

        let current_caret_position = self.current_caret_position;
        let params = lsproto::CodeActionParams {
            text_document: lsproto::TextDocumentIdentifier {
                uri: lsconv::file_name_to_document_uri(&self.active_filename),
            },
            range: lsproto::Range { end: current_caret_position, start: current_caret_position },
            context: lsproto::CodeActionContext { diagnostics, ..Default::default() },
            ..Default::default()
        };
        let result = self.send_request(t, lsproto::TEXT_DOCUMENT_CODE_ACTION_INFO, params);

        // Find all auto-import code actions (fixes with fixId/fixName related to imports)
        // Skip fix-all entries (those without diagnostics attached)
        let mut import_actions: Vec<lsproto::CodeAction> = Vec::new();
        if let Some(items) = result.command_or_code_action_array {
            for item in items {
                if let Some(code_action) = item.code_action
                    && code_action.kind == Some(lsproto::CodeActionKind::QUICK_FIX)
                    && code_action.diagnostics.as_ref().is_some_and(|d| !d.is_empty())
                {
                    import_actions.push(code_action);
                }
            }
        }

        if import_actions.is_empty() {
            if !expected_texts.is_empty() {
                t.fatal("No codefixes returned.");
            }
            return;
        }

        // Save the original content before any edits
        let script = self.get_script_info(&self.active_filename).unwrap();
        let original_content = script.content;
        // For each import action, apply it and check the result
        let mut actual_text_array: Vec<String> = Vec::with_capacity(import_actions.len());
        for action in &import_actions {
            // Apply the code action
            if let Some(changes) = action.edit.as_ref().and_then(|edit| edit.changes.as_ref()) {
                if changes.len() != 1 {
                    t.fatal(format!("Expected exactly 1 change, got {}", changes.len()));
                }
                for (uri, change_edits) in changes {
                    if *uri != lsconv::file_name_to_document_uri(&self.active_filename) {
                        t.fatal(format!(
                            "Expected change to file {}, got {}",
                            self.active_filename, uri.0
                        ));
                    }
                    let mut change_edits = change_edits.clone();
                    self.apply_text_edits(t, &mut change_edits);
                }
            }

            // Get the result text
            let text = match range_marker {
                Some(range_marker) => self.get_range_text(range_marker),
                None => self.get_script_info(&self.active_filename).unwrap().content,
            };
            actual_text_array.push(text);

            // Restore original content for next fix
            // Rust-only: Go's `script` is a pointer to the entry, so `len(script.content)` is the edited length.
            let edited_len = len(&self.get_script_info(&self.active_filename).unwrap().content);
            let active_filename = self.active_filename.clone();
            self.edit_script_and_update_markers(
                t,
                &active_filename,
                0,
                edited_len,
                &original_content,
            );
            self.current_caret_position = current_caret_position;
        }

        // Compare results
        if expected_texts.len() != actual_text_array.len() {
            let mut actual_joined = String::new();
            for (i, actual) in actual_text_array.iter().enumerate() {
                if i > 0 {
                    actual_joined.push_str("\n\n");
                    actual_joined.push_str(&"-".repeat(20));
                    actual_joined.push_str("\n\n");
                }
                actual_joined.push_str(actual);
            }
            t.fatal(format!(
                "Expected {} import fixes, got {}:\n\n{}",
                expected_texts.len(),
                actual_text_array.len(),
                actual_joined
            ));
        }
        for (i, &expected) in expected_texts.iter().enumerate() {
            let actual = &actual_text_array[i];
            if expected != actual {
                t.fatal(format!(
                    "Import fix at index {i} doesn't match.\n\nexpected: {expected:?}\nactual: {actual:?}"
                ));
            }
        }
    }

    /// Rust-only: `fourslash.go` `FourslashTest.getRangeText`.
    fn get_range_text(&self, r: &RangeMarker) -> String {
        let script = self.get_script_info(r.file_name()).unwrap();
        script.content[text_pos(r.range.pos()) as usize..text_pos(r.range.end()) as usize]
            .to_string()
    }

    /// Rust-only: `fourslash.go` `FourslashTest.VerifyDiagnostics`.
    pub fn verify_diagnostics(&self, t: &T, expected: &[lsproto::Diagnostic]) {
        self.verify_diagnostics_worker(t, expected, |_| true);
    }

    /// Rust-only: `fourslash.go` `FourslashTest.VerifyNonSuggestionDiagnostics`.
    // Similar to `VerifyDiagnostics`, but excludes suggestion diagnostics returned from server.
    pub fn verify_non_suggestion_diagnostics(&self, t: &T, expected: &[lsproto::Diagnostic]) {
        self.verify_diagnostics_worker(t, expected, |d| !is_suggestion_diagnostic(d));
    }

    /// Rust-only: `fourslash.go` `FourslashTest.verifyDiagnostics`. Go's `assertDeepEqual` with
    /// `diagnosticsIgnoreOpts` compares the diagnostics without their `Severity`, `Source` and
    /// `RelatedInformation`.
    fn verify_diagnostics_worker(
        &self,
        t: &T,
        expected: &[lsproto::Diagnostic],
        filter_diagnostics: impl Fn(&lsproto::Diagnostic) -> bool,
    ) {
        let active_filename = self.active_filename.clone();
        let actual_diagnostics = self.get_diagnostics(t, &active_filename);
        let actual_diagnostics: Vec<lsproto::Diagnostic> =
            actual_diagnostics.into_iter().filter(|d| filter_diagnostics(d)).collect();
        let empty_range = lsproto::Range::default();
        let mut expected_with_ranges: Vec<lsproto::Diagnostic> = Vec::with_capacity(expected.len());
        for diag in expected {
            if diag.range == empty_range {
                let ranges_in_file = self.get_ranges_in_file(&self.active_filename);
                if ranges_in_file.is_empty() {
                    t.fatal(format!(
                        "No ranges found in file {} to assign to diagnostic with empty range",
                        self.active_filename
                    ));
                }
                let mut diag_with_range = diag.clone();
                diag_with_range.range = ranges_in_file[0].ls_range;
                expected_with_ranges.push(diag_with_range);
            } else {
                expected_with_ranges.push(diag.clone());
            }
        }
        if actual_diagnostics.is_empty() && expected_with_ranges.is_empty() {
            return;
        }
        let ignore = |d: &lsproto::Diagnostic| lsproto::Diagnostic {
            severity: None,
            source: None,
            related_information: None,
            ..d.clone()
        };
        let actual: Vec<lsproto::Diagnostic> = actual_diagnostics.iter().map(ignore).collect();
        let expected: Vec<lsproto::Diagnostic> = expected_with_ranges.iter().map(ignore).collect();
        if actual != expected {
            t.fatal(format!(
                "Diagnostics do not match expected:\n--- actual\n{actual:#?}\n+++ expected\n{expected:#?}"
            ));
        }
    }

    /// Rust-only: `fourslash.go` `FourslashTest.VerifyErrorExistsAtRange`.
    // VerifyErrorExistsAtRange verifies that an error with the given code exists at the given range.
    pub fn verify_error_exists_at_range(
        &self,
        t: &T,
        range_marker: &RangeMarker,
        code: i64,
        message: &str,
    ) {
        let diagnostics = self.get_diagnostics(t, range_marker.file_name());
        for diag in &diagnostics {
            if let Some(integer) = diag.code.as_ref().and_then(|c| c.integer)
                && i64::from(integer) == code
            {
                // Check if the range matches
                if diag.range.start.line == range_marker.ls_range.start.line
                    && diag.range.start.character == range_marker.ls_range.start.character
                    && diag.range.end.line == range_marker.ls_range.end.line
                    && diag.range.end.character == range_marker.ls_range.end.character
                {
                    // If message is provided, verify it matches
                    if !message.is_empty() && diag.message.as_string() != message {
                        t.fatal(format!(
                            "Error at range has code {code} but message mismatch. Expected: {:?}, Got: {:?}",
                            message,
                            diag.message.as_string()
                        ));
                    }
                    return;
                }
            }
        }
        t.fatal(format!(
            "Expected error with code {code} at range {:?} but it was not found",
            range_marker.ls_range
        ));
    }
}

/// Rust-only: `fourslash.go` `ApplyCodeActionFromCompletionOptions`. Go's `*string` and `*lsutil.UserPreferences`
/// fields are `Option`s.
#[derive(Clone, Debug, Default)]
pub struct ApplyCodeActionFromCompletionOptions {
    pub name: String,
    pub source: String,
    pub auto_import_fix: Option<lsproto::AutoImportFix>,
    pub description: String,
    pub new_file_content: Option<String>,
    pub new_range_content: Option<String>,
    pub user_preferences: Option<UserPreferences>,
}

/// Rust-only: the completion code-action and import-fix module-specifier helpers of `fourslash.go` the Effect
/// auto-import tests use.
impl FourslashTest {
    /// Rust-only: `fourslash.go` `FourslashTest.findCompletionForCodeAction`.
    fn find_completion_for_code_action(
        &self,
        t: &T,
        items: &[lsproto::CompletionItem],
        description: &str,
    ) -> Option<lsproto::CompletionItem> {
        if let Some(item) = items
            .iter()
            .find(|item| item.additional_text_edits.as_ref().is_some_and(|edits| !edits.is_empty()))
        {
            return Some(item.clone());
        }
        items.iter().find_map(|item| {
            let resolved_item = self.resolve_completion_item(t, item);
            if resolved_item.additional_text_edits.as_ref().is_none_or(Vec::is_empty) {
                return None;
            }
            if !description.is_empty()
                && resolved_item.detail.as_ref().is_none_or(|detail| !detail.contains(description))
            {
                return None;
            }
            Some(resolved_item)
        })
    }

    /// Rust-only: `fourslash.go` `FourslashTest.resolveCompletionItem`.
    fn resolve_completion_item(
        &self,
        t: &T,
        item: &lsproto::CompletionItem,
    ) -> lsproto::CompletionItem {
        self.send_request(t, lsproto::COMPLETION_ITEM_RESOLVE_INFO, item.clone())
    }

    /// Rust-only: `fourslash.go` `FourslashTest.VerifyApplyCodeActionFromCompletion`. Go's `markerName` pointer is
    /// a `&str` (Go dereferences it unconditionally).
    pub fn verify_apply_code_action_from_completion(
        &mut self,
        t: &T,
        marker_name: &str,
        options: &ApplyCodeActionFromCompletionOptions,
    ) {
        self.go_to_marker(t, marker_name);
        let user_preferences = match &options.user_preferences {
            Some(user_preferences) => user_preferences.clone(),
            // Default preferences: enables auto-imports
            None => lsutil::new_default_user_preferences(),
        };

        let reset = self.configure_with_reset(t, user_preferences);
        // Rust-only: Go's `defer reset()`: the original preferences are restored at the end.
        self.verify_apply_code_action_from_completion_worker(t, options);
        self.configure(t, reset);
    }

    /// Rust-only: the body of `VerifyApplyCodeActionFromCompletion` after its preferences are configured.
    fn verify_apply_code_action_from_completion_worker(
        &mut self,
        t: &T,
        options: &ApplyCodeActionFromCompletionOptions,
    ) {
        let Some(completions_list) = self.get_completions(t, None /*userPreferences*/) else {
            t.fatal("Expected a completion list but got nil");
        }; // Already configured, so we do not need to pass it in again
        let items: Vec<lsproto::CompletionItem> = completions_list
            .items
            .into_iter()
            .filter(|item| {
                let Some(data) = &item.data else {
                    return false;
                };
                if item.label != options.name {
                    return false;
                }

                if let Some(auto_import_fix) = &options.auto_import_fix {
                    let mut module_specifier = auto_import_fix.module_specifier.as_str();
                    if module_specifier.is_empty() {
                        module_specifier = &options.source;
                    }
                    return data.auto_import.as_ref().is_some_and(|auto_import| {
                        auto_import.module_specifier == module_specifier
                    });
                }
                if data.auto_import.is_none()
                    && !data.source.is_empty()
                    && data.source == options.source
                {
                    return true;
                }
                if data
                    .auto_import
                    .as_ref()
                    .is_some_and(|auto_import| auto_import.module_specifier == options.source)
                {
                    return true;
                }
                false
            })
            .collect();

        let Some(item) = self.find_completion_for_code_action(t, &items, &options.description)
        else {
            t.fatal(format!(
                "Code action '{}' from source '{}' not found.",
                options.name, options.source
            ));
        };

        // apply the item to the test files
        let mut edits = item.additional_text_edits.unwrap_or_default();
        self.apply_text_edits(t, &mut edits);
        if let Some(new_file_content) = &options.new_file_content {
            let content = self.get_script_info(&self.active_filename).unwrap().content;
            if content != *new_file_content {
                t.fatal(format!(
                    "File content after applying code action did not match expected content.\n\nexpected: {new_file_content:?}\nactual: {content:?}"
                ));
            }
        } else if options.new_range_content.is_some() {
            t.fatal("!!! TODO");
        }
    }

    /// Rust-only: `fourslash.go` `FourslashTest.VerifyImportFixModuleSpecifiers`. Go's `preferences` pointer is an
    /// `Option`.
    pub fn verify_import_fix_module_specifiers(
        &mut self,
        t: &T,
        marker_name: &str,
        expected_module_specifiers: &[&str],
        preferences: Option<&UserPreferences>,
    ) {
        self.go_to_marker(t, marker_name);

        // Rust-only: Go's `defer reset()`: the original preferences are restored at the end.
        let reset =
            preferences.map(|preferences| self.configure_with_reset(t, preferences.clone()));
        self.verify_import_fix_module_specifiers_worker(t, expected_module_specifiers);
        if let Some(reset) = reset {
            self.configure(t, reset);
        }
    }

    /// Rust-only: the body of `VerifyImportFixModuleSpecifiers` after its preferences are configured.
    fn verify_import_fix_module_specifiers_worker(
        &self,
        t: &T,
        expected_module_specifiers: &[&str],
    ) {
        // Get diagnostics at the current position to find errors that need import fixes
        let diag_params = lsproto::DocumentDiagnosticParams {
            text_document: lsproto::TextDocumentIdentifier {
                uri: lsconv::file_name_to_document_uri(&self.active_filename),
            },
            ..Default::default()
        };
        let diag_result = self.send_request(t, lsproto::TEXT_DOCUMENT_DIAGNOSTIC_INFO, diag_params);

        let mut diagnostics: Vec<lsproto::Diagnostic> = Vec::new();
        if let Some(report) = diag_result.full_document_diagnostic_report {
            diagnostics = report.items;
        }

        let params = lsproto::CodeActionParams {
            text_document: lsproto::TextDocumentIdentifier {
                uri: lsconv::file_name_to_document_uri(&self.active_filename),
            },
            range: lsproto::Range {
                start: self.current_caret_position,
                end: self.current_caret_position,
            },
            context: lsproto::CodeActionContext { diagnostics, ..Default::default() },
            ..Default::default()
        };
        let result = self.send_request(t, lsproto::TEXT_DOCUMENT_CODE_ACTION_INFO, params);

        // Extract module specifiers from import fix code actions
        let mut actual_module_specifiers: Vec<String> = Vec::new();
        if let Some(items) = &result.command_or_code_action_array {
            for item in items {
                if let Some(code_action) = &item.code_action
                    && code_action.kind == Some(lsproto::CodeActionKind::QUICK_FIX)
                    && let Some(changes) =
                        code_action.edit.as_ref().and_then(|edit| edit.changes.as_ref())
                {
                    for change_edits in changes.values() {
                        for edit in change_edits {
                            let module_spec = extract_module_specifier(&edit.new_text);
                            if !module_spec.is_empty()
                                && !actual_module_specifiers.iter().any(|s| s == module_spec)
                            {
                                actual_module_specifiers.push(module_spec.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Compare results
        if actual_module_specifiers.len() != expected_module_specifiers.len() {
            t.fatal(format!(
                "Expected {} module specifiers, got {}.\nExpected: {:?}\nActual: {:?}",
                expected_module_specifiers.len(),
                actual_module_specifiers.len(),
                expected_module_specifiers,
                actual_module_specifiers
            ));
        }

        for (i, &expected) in expected_module_specifiers.iter().enumerate() {
            if i >= actual_module_specifiers.len() || actual_module_specifiers[i] != expected {
                t.fatal(format!(
                    "Module specifier mismatch at index {i}.\nExpected: {expected_module_specifiers:?}\nActual: {actual_module_specifiers:?}"
                ));
            }
        }
    }
}

/// Rust-only: `fourslash.go` `extractModuleSpecifier`.
fn extract_module_specifier(text: &str) -> &str {
    // Try to match: from "..." or from '...'
    // Try to match: require("...") or require('...')
    for (prefix, quote) in
        [("from \"", '"'), ("from '", '\''), ("require(\"", '"'), ("require('", '\'')]
    {
        if let Some(idx) = text.find(prefix) {
            let start = idx + prefix.len();
            if let Some(end) = text[start..].find(quote) {
                return &text[start..start + end];
            }
        }
    }
    ""
}

impl Drop for FourslashTest {
    fn drop(&mut self) {
        if let Some(close_client) = self.close_client.take() {
            let _ = close_client.close();
        }
    }
}

/// Rust-only: `fourslash.go` `FourslashTest.verifyHoverContent` (it does not use the test's state).
pub fn verify_hover_content(
    t: &T,
    actual: &lsproto::MarkupContentOrStringOrMarkedStringWithLanguageOrMarkedStrings,
    expected_text: &str,
    expected_documentation: &str,
    prefix: &str,
) {
    match &actual.markup_content {
        Some(markup_content) => verify_hover_markdown(
            t,
            &markup_content.value,
            expected_text,
            expected_documentation,
            prefix,
        ),
        None => t.fatal(format!("{prefix}Expected markup content, got: {actual:?}")),
    }
}

/// Rust-only: `fourslash.go` `FourslashTest.verifyHoverMarkdown` (`assertDeepEqual` fails with both values).
pub fn verify_hover_markdown(
    t: &T,
    actual: &str,
    expected_text: &str,
    expected_documentation: &str,
    prefix: &str,
) {
    let expected = format!("```typescript\n{expected_text}\n```\n{expected_documentation}");
    if actual != expected {
        t.fatal(format!(
            "{prefix}Hover markdown content mismatch:\n--- expected\n{expected}\n+++ actual\n{actual}"
        ));
    }
}

/// Rust-only: Go's `len(s)` as an `int`.
fn len(s: &str) -> i64 {
    i64::try_from(s.len()).unwrap()
}

/// Rust-only: a Go `int` offset as a tsrs position. Go's `updatePosition` marks a position inside an edit with
/// `-1`; tsrs positions are unsigned, so such a position (of a marker no Effect runner reads again) becomes 0.
fn to_u32(offset: i64) -> u32 {
    u32::try_from(offset).unwrap_or(0)
}

/// Rust-only: `fourslash.go` `updatePosition`.
fn update_position(pos: i64, edit_start: i64, edit_end: i64, new_text: &str) -> i64 {
    if pos <= edit_start {
        return pos;
    }
    // If inside the edit, return -1 to mark as invalid
    if pos < edit_end {
        return -1;
    }
    pos + len(new_text) - (edit_end - edit_start)
}

/// Rust-only: `fourslash.go` `getLanguageKind`.
fn get_language_kind(filename: &str) -> lsproto::LanguageKind {
    use ets_shim::core::tspath::*;
    if file_extension_is_one_of(
        filename,
        &[
            EXTENSION_TS,
            EXTENSION_MTS,
            EXTENSION_CTS,
            EXTENSION_DMTS,
            EXTENSION_DCTS,
            EXTENSION_DTS,
        ],
    ) {
        return lsproto::LanguageKind::TYPE_SCRIPT;
    }
    if file_extension_is_one_of(filename, &[EXTENSION_JS, EXTENSION_MJS, EXTENSION_CJS]) {
        return lsproto::LanguageKind::JAVA_SCRIPT;
    }
    if file_extension_is(filename, EXTENSION_JSX) {
        return lsproto::LanguageKind::JAVA_SCRIPT_REACT;
    }
    if file_extension_is(filename, EXTENSION_TSX) {
        return lsproto::LanguageKind::TYPE_SCRIPT_REACT;
    }
    if file_extension_is(filename, EXTENSION_JSON) {
        return lsproto::LanguageKind::JSON;
    }
    lsproto::LanguageKind::TYPE_SCRIPT // !!! should we error in this case?
}

/// Rust-only: `fourslash.go` `defaultSemanticTokenTypes`.
fn default_semantic_token_types() -> Vec<String> {
    use lsproto::SemanticTokenType as S;
    [
        S::NAMESPACE,
        S::CLASS,
        S::ENUM,
        S::INTERFACE,
        S::STRUCT,
        S::TYPE_PARAMETER,
        S::TYPE,
        S::PARAMETER,
        S::VARIABLE,
        S::PROPERTY,
        S::ENUM_MEMBER,
        S::DECORATOR,
        S::EVENT,
        S::FUNCTION,
        S::METHOD,
        S::MACRO,
        S::LABEL,
        S::COMMENT,
        S::STRING,
        S::KEYWORD,
        S::NUMBER,
        S::REGEXP,
        S::OPERATOR,
    ]
    .into_iter()
    .map(|s| s.0.to_string())
    .collect()
}

/// Rust-only: `fourslash.go` `defaultSemanticTokenModifiers`.
fn default_semantic_token_modifiers() -> Vec<String> {
    use lsproto::SemanticTokenModifier as S;
    let mut modifiers: Vec<String> = [
        S::DECLARATION,
        S::DEFINITION,
        S::READONLY,
        S::STATIC,
        S::DEPRECATED,
        S::ABSTRACT,
        S::ASYNC,
        S::MODIFICATION,
        S::DOCUMENTATION,
        S::DEFAULT_LIBRARY,
    ]
    .into_iter()
    .map(|s| s.0.to_string())
    .collect();
    modifiers.push("local".to_string());
    modifiers
}

/// Rust-only: `fourslash.go` `getCapabilitiesWithDefaults` (with the `default*Capabilities` values it uses).
fn get_capabilities_with_defaults(
    capabilities: Option<lsproto::ClientCapabilities>,
) -> lsproto::ClientCapabilities {
    let ptr_true = Some(true);
    let ptr_false = Some(false);
    let markdown_and_plain_text =
        Some(vec![lsproto::MarkupKind::MARKDOWN, lsproto::MarkupKind::PLAIN_TEXT]);
    let diagnostic_tags = || lsproto::ClientDiagnosticsTagOptions {
        value_set: vec![lsproto::DiagnosticTag::UNNECESSARY, lsproto::DiagnosticTag::DEPRECATED],
    };

    let mut capabilities_with_defaults = capabilities.unwrap_or_default();
    capabilities_with_defaults.general = Some(lsproto::GeneralClientCapabilities {
        position_encodings: Some(vec![lsproto::PositionEncodingKind::UTF8]),
        ..Default::default()
    });
    if capabilities_with_defaults.experimental.is_none() {
        capabilities_with_defaults.experimental =
            Some(lsproto::ExperimentalClientCapabilities { hover_verbosity_level: ptr_true });
    }
    let text_document =
        capabilities_with_defaults.text_document.get_or_insert_with(Default::default);
    if text_document.completion.is_none() {
        text_document.completion = Some(lsproto::CompletionClientCapabilities {
            completion_item: Some(lsproto::ClientCompletionItemOptions {
                snippet_support: ptr_false,
                commit_characters_support: ptr_true,
                preselect_support: ptr_true,
                label_details_support: ptr_false,
                insert_replace_support: ptr_true,
                documentation_format: markdown_and_plain_text.clone(),
                ..Default::default()
            }),
            completion_list: Some(lsproto::CompletionListCapabilities {
                item_defaults: Some(vec!["commitCharacters".to_string(), "editRange".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        });
    }
    if text_document.diagnostic.is_none() {
        text_document.diagnostic = Some(lsproto::DiagnosticClientCapabilities {
            related_information: ptr_true,
            tag_support: Some(diagnostic_tags()),
            ..Default::default()
        });
    }
    if text_document.publish_diagnostics.is_none() {
        text_document.publish_diagnostics = Some(lsproto::PublishDiagnosticsClientCapabilities {
            related_information: ptr_true,
            tag_support: Some(diagnostic_tags()),
            ..Default::default()
        });
    }
    if text_document.semantic_tokens.is_none() {
        text_document.semantic_tokens = Some(lsproto::SemanticTokensClientCapabilities {
            requests: lsproto::ClientSemanticTokensRequestOptions {
                full: Some(lsproto::BooleanOrClientSemanticTokensRequestFullDelta {
                    boolean: ptr_true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            token_types: default_semantic_token_types(),
            token_modifiers: default_semantic_token_modifiers(),
            formats: vec![lsproto::TokenFormat::RELATIVE],
            ..Default::default()
        });
    }
    if text_document.definition.is_none() {
        text_document.definition = Some(lsproto::DefinitionClientCapabilities {
            link_support: ptr_true,
            ..Default::default()
        });
    }
    if text_document.type_definition.is_none() {
        text_document.type_definition = Some(lsproto::TypeDefinitionClientCapabilities {
            link_support: ptr_true,
            ..Default::default()
        });
    }
    if text_document.implementation.is_none() {
        text_document.implementation = Some(lsproto::ImplementationClientCapabilities {
            link_support: ptr_true,
            ..Default::default()
        });
    }
    if text_document.hover.is_none() {
        text_document.hover = Some(lsproto::HoverClientCapabilities {
            content_format: markdown_and_plain_text.clone(),
            ..Default::default()
        });
    }
    if text_document.signature_help.is_none() {
        text_document.signature_help = Some(lsproto::SignatureHelpClientCapabilities {
            signature_information: Some(lsproto::ClientSignatureInformationOptions {
                documentation_format: markdown_and_plain_text,
                parameter_information: Some(lsproto::ClientSignatureParameterInformationOptions {
                    label_offset_support: ptr_true,
                }),
                active_parameter_support: ptr_true,
                ..Default::default()
            }),
            context_support: ptr_true,
            ..Default::default()
        });
    }
    if text_document.document_symbol.is_none() {
        text_document.document_symbol = Some(lsproto::DocumentSymbolClientCapabilities {
            hierarchical_document_symbol_support: ptr_true,
            ..Default::default()
        });
    }
    if text_document.folding_range.is_none() {
        text_document.folding_range = Some(lsproto::FoldingRangeClientCapabilities {
            range_limit: Some(5000),
            // LineFoldingOnly: ptrTrue,
            folding_range_kind: Some(lsproto::ClientFoldingRangeKindOptions {
                value_set: Some(vec![
                    lsproto::FoldingRangeKind::COMMENT,
                    lsproto::FoldingRangeKind::IMPORTS,
                    lsproto::FoldingRangeKind::REGION,
                ]),
            }),
            folding_range: Some(lsproto::ClientFoldingRangeOptions {
                collapsed_text: ptr_true, // Unused by our testing, but set to exercise the code.
            }),
            ..Default::default()
        });
    }
    let workspace = capabilities_with_defaults.workspace.get_or_insert_with(Default::default);
    if workspace.file_operations.is_none() {
        workspace.file_operations = Some(lsproto::FileOperationClientCapabilities {
            will_rename: ptr_true,
            ..Default::default()
        });
    }
    if workspace.workspace_edit.is_none() {
        workspace.workspace_edit = Some(lsproto::WorkspaceEditClientCapabilities {
            document_changes: ptr_true,
            resource_operations: Some(vec![lsproto::ResourceOperationKind::RENAME]),
            ..Default::default()
        });
    }
    if workspace.configuration.is_none() {
        workspace.configuration = ptr_true;
    }
    capabilities_with_defaults
}

// ---------------------------------------------------------------------------------------------------------------
// The Effect additions of _patches/typescript/010-fourslash-fourslash.patch.
// ---------------------------------------------------------------------------------------------------------------

/// Rust-only: `010-fourslash-fourslash.patch` `QuickFixInfo`.
// QuickFixInfo holds a diagnostic and the titles of its available quick fixes.
#[derive(Clone, Debug)]
pub struct QuickFixInfo {
    pub diagnostic: lsproto::Diagnostic,
    pub fix_titles: Vec<String>,
}

/// Rust-only: `010-fourslash-fourslash.patch` `FileChange`.
// FileChange holds the before/after content for a single file affected by a quick fix.
#[derive(Clone, Debug, Default)]
pub struct FileChange {
    pub uri: lsproto::DocumentUri,
    pub before: String,
    pub after: String,
    pub created: bool,
    pub deleted: bool,
}

/// Rust-only: `010-fourslash-fourslash.patch` `AppliedQuickFixResult`.
// AppliedQuickFixResult holds the result of applying a single quick fix.
#[derive(Clone, Debug)]
pub struct AppliedQuickFixResult {
    pub diagnostic: lsproto::Diagnostic,
    pub fix_index: usize,
    pub fix_title: String,
    pub changes: Vec<FileChange>,
}

/// Rust-only: `010-fourslash-fourslash.patch` `RefactorActionInfo`.
// RefactorActionInfo holds information about a single refactor code action.
#[derive(Clone, Debug)]
pub struct RefactorActionInfo {
    pub title: String,
    pub kind: String,
}

/// Rust-only: `010-fourslash-fourslash.patch` `AppliedRefactorResult`.
// AppliedRefactorResult holds the result of applying a single refactor action.
#[derive(Clone, Debug)]
pub struct AppliedRefactorResult {
    pub action_index: usize,
    pub action_title: String,
    pub changes: Vec<FileChange>,
}

/// Rust-only: the per-file undo state of `ApplyQuickFix` / `ApplyRefactorAction` (Go's local `fileEditState`).
struct FileEditState {
    file_name: String,
    original_content: String,
}

/// Rust-only: Go's `item.CodeAction != nil && item.CodeAction.Kind != nil && *item.CodeAction.Kind == kind`.
fn is_code_action_of_kind(
    item: &lsproto::CommandOrCodeAction,
    kind: lsproto::CodeActionKind,
) -> bool {
    item.code_action.as_ref().is_some_and(|action| action.kind == Some(kind))
}

/// Rust-only: Go's `strings.HasPrefix(string(*item.CodeAction.Kind), "refactor.")` check.
fn is_refactor_code_action(item: &lsproto::CommandOrCodeAction) -> bool {
    item.code_action
        .as_ref()
        .and_then(|action| action.kind)
        .is_some_and(|kind| kind.0.starts_with("refactor."))
}

impl FourslashTest {
    /// Rust-only: `010-fourslash-fourslash.patch` `FourslashTest.GetQuickFixesForDiagnostics`.
    // GetQuickFixesForDiagnostics returns an inventory of quick fixes available for all
    // diagnostics in the given file. It sends a diagnostic request followed by per-diagnostic
    // code action requests, filtering to CodeActionKindQuickFix.
    pub fn get_quick_fixes_for_diagnostics(
        &self,
        t: &T,
        file_uri: &lsproto::DocumentUri,
    ) -> Vec<QuickFixInfo> {
        // Get diagnostics
        let diag_params = lsproto::DocumentDiagnosticParams {
            text_document: lsproto::TextDocumentIdentifier { uri: file_uri.clone() },
            ..Default::default()
        };
        let diag_result = self.send_request(t, lsproto::TEXT_DOCUMENT_DIAGNOSTIC_INFO, diag_params);

        let mut diagnostics: Vec<lsproto::Diagnostic> = Vec::new();
        if let Some(report) = diag_result.full_document_diagnostic_report {
            diagnostics = report.items;
        }

        if diagnostics.is_empty() {
            return Vec::new();
        }

        // Sort diagnostics deterministically by position then code
        ets_go::slices::sort_stable_func(&mut diagnostics, |a, b| {
            let c = i64::from(a.range.start.line) - i64::from(b.range.start.line);
            if c != 0 {
                return c.cmp(&0);
            }
            let c = i64::from(a.range.start.character) - i64::from(b.range.start.character);
            if c != 0 {
                return c.cmp(&0);
            }
            let ac = a.code.as_ref().and_then(|c| c.integer).unwrap_or(0);
            let bc = b.code.as_ref().and_then(|c| c.integer).unwrap_or(0);
            (i64::from(ac) - i64::from(bc)).cmp(&0)
        });

        let mut result = Vec::new();
        for diag in diagnostics {
            // Request code actions for this specific diagnostic
            let params = lsproto::CodeActionParams {
                text_document: lsproto::TextDocumentIdentifier { uri: file_uri.clone() },
                range: diag.range,
                context: lsproto::CodeActionContext {
                    diagnostics: vec![diag.clone()],
                    only: Some(vec![lsproto::CodeActionKind::QUICK_FIX]),
                    ..Default::default()
                },
                ..Default::default()
            };
            let code_actions =
                self.send_request(t, lsproto::TEXT_DOCUMENT_CODE_ACTION_INFO, params);

            let mut titles = Vec::new();
            if let Some(items) = &code_actions.command_or_code_action_array {
                for item in items {
                    if is_code_action_of_kind(item, lsproto::CodeActionKind::QUICK_FIX) {
                        titles.push(item.code_action.as_ref().unwrap().title.clone());
                    }
                }
            }

            result.push(QuickFixInfo { diagnostic: diag, fix_titles: titles });
        }

        result
    }

    /// Rust-only: `010-fourslash-fourslash.patch` `FourslashTest.ApplyQuickFix`.
    // ApplyQuickFix applies a specific quick fix for a diagnostic and returns the before/after
    // content for all affected files. After capturing the result, it undoes the changes to
    // restore the original state.
    pub fn apply_quick_fix(
        &mut self,
        t: &T,
        file_uri: &lsproto::DocumentUri,
        diagnostic: &lsproto::Diagnostic,
        fix_index: usize,
    ) -> AppliedQuickFixResult {
        // Request code actions for this diagnostic
        let params = lsproto::CodeActionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: file_uri.clone() },
            range: diagnostic.range,
            context: lsproto::CodeActionContext {
                diagnostics: vec![diagnostic.clone()],
                only: Some(vec![lsproto::CodeActionKind::QUICK_FIX]),
                ..Default::default()
            },
            ..Default::default()
        };
        let code_actions = self.send_request(t, lsproto::TEXT_DOCUMENT_CODE_ACTION_INFO, params);

        // Filter to quick fixes only
        let quick_fixes: Vec<lsproto::CodeAction> = code_actions
            .command_or_code_action_array
            .unwrap_or_default()
            .into_iter()
            .filter(|item| is_code_action_of_kind(item, lsproto::CodeActionKind::QUICK_FIX))
            .filter_map(|item| item.code_action)
            .collect();

        if fix_index >= quick_fixes.len() {
            t.fatal(format!(
                "Quick fix index {fix_index} out of range (have {} fixes)",
                quick_fixes.len()
            ));
        }

        let action = &quick_fixes[fix_index];
        let mut result = AppliedQuickFixResult {
            diagnostic: diagnostic.clone(),
            fix_index,
            fix_title: action.title.clone(),
            changes: Vec::new(),
        };

        result.changes = self.apply_workspace_edit_and_undo(t, action);
        result
    }

    /// Rust-only: `010-fourslash-fourslash.patch` `FourslashTest.GetRefactorActionsForRange`.
    // GetRefactorActionsForRange returns refactor actions available for the given selection range.
    // It sends a code action request with refactor.* kinds and returns matching actions.
    pub fn get_refactor_actions_for_range(
        &self,
        t: &T,
        file_uri: &lsproto::DocumentUri,
        selection_range: lsproto::Range,
    ) -> Vec<RefactorActionInfo> {
        let params = lsproto::CodeActionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: file_uri.clone() },
            range: selection_range,
            context: lsproto::CodeActionContext {
                only: Some(vec![lsproto::CodeActionKind::REFACTOR]),
                ..Default::default()
            },
            ..Default::default()
        };
        let code_actions = self.send_request(t, lsproto::TEXT_DOCUMENT_CODE_ACTION_INFO, params);

        let mut result = Vec::new();
        if let Some(items) = &code_actions.command_or_code_action_array {
            for item in items {
                if is_refactor_code_action(item) {
                    let action = item.code_action.as_ref().unwrap();
                    result.push(RefactorActionInfo {
                        title: action.title.clone(),
                        kind: action.kind.unwrap().0.to_string(),
                    });
                }
            }
        }
        result
    }

    /// Rust-only: `010-fourslash-fourslash.patch` `FourslashTest.ApplyRefactorAction`.
    // ApplyRefactorAction applies a specific refactor action for a selection range and returns
    // the before/after content for all affected files. After capturing the result, it undoes
    // the changes to restore the original state.
    pub fn apply_refactor_action(
        &mut self,
        t: &T,
        file_uri: &lsproto::DocumentUri,
        selection_range: lsproto::Range,
        action_index: usize,
    ) -> AppliedRefactorResult {
        let params = lsproto::CodeActionParams {
            text_document: lsproto::TextDocumentIdentifier { uri: file_uri.clone() },
            range: selection_range,
            context: lsproto::CodeActionContext {
                only: Some(vec![lsproto::CodeActionKind::REFACTOR]),
                ..Default::default()
            },
            ..Default::default()
        };
        let code_actions = self.send_request(t, lsproto::TEXT_DOCUMENT_CODE_ACTION_INFO, params);

        // Filter to refactor actions
        let refactor_actions: Vec<lsproto::CodeAction> = code_actions
            .command_or_code_action_array
            .unwrap_or_default()
            .into_iter()
            .filter(is_refactor_code_action)
            .filter_map(|item| item.code_action)
            .collect();

        if action_index >= refactor_actions.len() {
            t.fatal(format!(
                "Refactor action index {action_index} out of range (have {} actions)",
                refactor_actions.len()
            ));
        }

        let action = &refactor_actions[action_index];
        let mut result = AppliedRefactorResult {
            action_index,
            action_title: action.title.clone(),
            changes: Vec::new(),
        };

        result.changes = self.apply_workspace_edit_and_undo(t, action);
        result
    }

    /// Rust-only: the part `ApplyQuickFix` and `ApplyRefactorAction` share verbatim in the Go patch: apply the
    /// action's edits file by file (sorted by URI), record each file's before and after content, then undo the
    /// changes in reverse order.
    fn apply_workspace_edit_and_undo(
        &mut self,
        t: &T,
        action: &lsproto::CodeAction,
    ) -> Vec<FileChange> {
        let mut changes_result = Vec::new();
        let Some(changes) = action.edit.as_ref().and_then(|edit| edit.changes.as_ref()) else {
            return changes_result;
        };
        if changes.is_empty() {
            return changes_result;
        }

        // Collect URIs in sorted order for deterministic output
        let mut uris: Vec<lsproto::DocumentUri> = changes.keys().cloned().collect();
        ets_go::slices::sort_func(&mut uris, |a, b| a.0.cmp(&b.0));

        // Save original state and apply edits per file
        let mut edit_states: Vec<FileEditState> = Vec::new();

        for uri in uris {
            let mut edits = changes[&uri].clone();
            let file_name = uri.file_name();
            let script = self.get_script_info(&file_name);

            let mut before_content = String::new();
            let mut created = false;
            match &script {
                None => created = true,
                Some(script) => before_content.clone_from(&script.content),
            }

            // Save state for undo
            edit_states.push(FileEditState {
                file_name: file_name.clone(),
                original_content: before_content.clone(),
            });

            // Apply edits
            let saved_active = std::mem::replace(&mut self.active_filename, file_name.clone());

            if script.is_none() {
                // File doesn't exist yet - create it with empty content
                self.script_infos
                    .lock()
                    .insert(file_name.clone(), new_script_info_from_file_name(&file_name, ""));
            }

            self.apply_text_edits(t, &mut edits);
            self.active_filename = saved_active;

            let after_content = self.get_script_info(&file_name).unwrap().content;

            changes_result.push(FileChange {
                uri,
                before: before_content,
                after: after_content,
                created,
                deleted: false,
            });
        }

        // Undo all changes in reverse order, notifying the LSP server
        for state in edit_states.iter().rev() {
            let script = self.get_script_info(&state.file_name);
            if let Some(script) = script {
                if state.original_content.is_empty() {
                    // File was created - remove it
                    self.script_infos.lock().remove(&state.file_name);
                } else {
                    // Restore original content via editScript so the server is notified
                    self.edit_script(
                        t,
                        &state.file_name,
                        &TextChange {
                            text_range: TextRange::new(
                                0,
                                u32::try_from(script.content.len()).unwrap(),
                            ),
                            new_text: state.original_content.clone(),
                        },
                    );
                }
            }
        }

        changes_result
    }
}
