// Package etsdiagnostics implements the native Effect diagnostics CLI mode.

use std::fmt::Write as _;
use std::io::Write;
use std::sync::Arc;

use ets_etscore::{AnyValue, EFFECT_PLUGIN_NAME, EffectPluginOptions};
use ets_shim::P;
use ets_shim::ast::Diagnostic;
use ets_shim::core::collections::Set;
use ets_shim::core::context::{self, CheckerLifetime, Context};
use ets_shim::core::tspath;
use ets_shim::diagnostics as tsdiag;
use ets_shim::project::{self, APISnapshotRequest, FileChangeSummary, Session, Snapshot};
use ets_shim::vfs::{FS, bundled, osvfs};
use rustc_hash::{FxHashMap, FxHashSet};

// go: etsdiagnostics/diagnostics.go:30 noFilesMessage aa808da56cf61f5d
pub(crate) const NO_FILES_MESSAGE: &str =
    "No files to check. Please provide an existing .ts file or a project tsconfig.json";

// go: etsdiagnostics/diagnostics.go:32 request 43d8cd7c61cba8c6
// Rust: decoded by `decode_request` (Go's `encoding/json` into this struct; `json:` names in the comments).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Request {
    pub(crate) cwd: String,                // "cwd"
    pub(crate) file: String,               // "file,omitempty"
    pub(crate) project: String,            // "project,omitempty"
    pub(crate) format: String,             // "format"
    pub(crate) strict: bool,               // "strict"
    pub(crate) severity: String,           // "severity,omitempty"
    pub(crate) progress: bool,             // "progress"
    pub(crate) lsp_config: Option<String>, // "lspconfig,omitempty"
    pub(crate) list_files: bool,           // "listFiles,omitempty"
}

// go: etsdiagnostics/diagnostics.go:44 severity a924ad7b5ee88336
// Rust: Go's `string` type with three constants; only those values are ever stored (`parseSeverityFilter` keeps
// only them, `categoryToSeverity` returns only them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Severity {
    Error,
    Warning,
    Message,
}

// go: etsdiagnostics/diagnostics.go:46 severityError 5aff3e7e5da30e81
impl Severity {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Message => "message",
        }
    }

    /// Rust-only: Go's `severity(s)` conversion compared against the constants.
    fn from_str(s: &str) -> Option<Severity> {
        match s {
            "error" => Some(Severity::Error),
            "warning" => Some(Severity::Warning),
            "message" => Some(Severity::Message),
            _ => None,
        }
    }
}

// go: etsdiagnostics/diagnostics.go:52 location 50491cddf505978d
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Location {
    pub(crate) file: String,    // "file"
    pub(crate) start: i64,      // "start"
    pub(crate) length: i64,     // "length"
    pub(crate) line: i64,       // "line"
    pub(crate) column: i64,     // "column"
    pub(crate) end_line: i64,   // "endLine"
    pub(crate) end_column: i64, // "endColumn"
    pub(crate) source: &'static str,
}

// go: etsdiagnostics/diagnostics.go:63 formattedDiagnostic 047c951f41cf078d
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FormattedDiagnostic {
    pub(crate) location: Location,
    pub(crate) severity: Severity, // "severity"
    pub(crate) code: i32,          // "code"
    pub(crate) name: String,       // "name"
    pub(crate) message: String,    // "message"
    pub(crate) related_information: Vec<RelatedInformation>, // "relatedInformation,omitempty"
}

// go: etsdiagnostics/diagnostics.go:72 relatedInformation 0aa8f425477e2528
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RelatedInformation {
    pub(crate) location: Location,
    pub(crate) code: i32,       // "code"
    pub(crate) message: String, // "message"
}

// go: etsdiagnostics/diagnostics.go:78 summary bd471d8b176e7188
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Summary {
    pub(crate) files_checked: i64,        // "filesChecked"
    pub(crate) files_without_plugin: i64, // "filesWithoutPlugin"
    pub(crate) total_files: i64,          // "totalFiles"
    pub(crate) errors: i64,               // "errors"
    pub(crate) warnings: i64,             // "warnings"
    pub(crate) messages: i64,             // "messages"
}

// go: etsdiagnostics/diagnostics.go:87 fileEffectVersion d91a0860ba2ef491
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileEffectVersion {
    pub(crate) file: String,             // "file"
    pub(crate) detected_effect: String,  // "detectedEffect"
    pub(crate) supported_effect: String, // "supportedEffect"
}

// go: etsdiagnostics/diagnostics.go:93 jsonOutput 44f4f1745ebcaacc
// Rust: encoded by `encode_json_output` (Go's `json.Encoder` with a two-space indent).
pub(crate) struct JsonOutput<'a> {
    pub(crate) diagnostics: &'a [FormattedDiagnostic], // "diagnostics"
    pub(crate) files: &'a [FileEffectVersion],         // "files,omitempty"
    pub(crate) summary: Summary,                       // "summary"
}

// go: etsdiagnostics/diagnostics.go:100 Run f83240e0aa9c9d21
// Run decodes and executes one JSON diagnostics request.
pub fn run(ctx: &Context, args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if args.len() != 1 {
        let _ = writeln!(stderr, "expected one JSON request after --effect-cli-diagnostics");
        return 2;
    }

    let mut req = match decode_request(&args[0]) {
        Ok(req) => req,
        Err(err) => {
            let _ = writeln!(stderr, "invalid diagnostics request: {err}");
            return 2;
        }
    };
    if req.cwd.is_empty() {
        let _ = writeln!(stderr, "diagnostics request is missing cwd");
        return 2;
    }
    if req.format.is_empty() {
        req.format = "pretty".to_string();
    }
    if req.format != "json"
        && req.format != "pretty"
        && req.format != "text"
        && req.format != "github-actions"
    {
        let _ = writeln!(stderr, "unsupported diagnostics format {}", go_quote(&req.format));
        return 2;
    }

    let Ok((r#override, override_provided)) = parse_lsp_config(req.lsp_config.as_deref()) else {
        let _ = writeln!(
            stderr,
            "Invalid JSON lsp config: {}",
            req.lsp_config.as_deref().unwrap_or_default()
        );
        return 1;
    };

    let (diagnostics, files, result_summary) =
        match collect(ctx, &req, r#override.as_ref(), override_provided, stderr) {
            Ok(result) => result,
            Err(err) => {
                let _ = writeln!(stderr, "{err}");
                return 1;
            }
        };
    if let Err(err) = write_output(stdout, &req.format, &diagnostics, &files, result_summary) {
        let _ = writeln!(stderr, "unable to write diagnostics: {err}");
        return 2;
    }
    if result_summary.errors > 0 || (req.strict && result_summary.warnings > 0) {
        return 1;
    }
    0
}

// go: etsdiagnostics/diagnostics.go:144 parseLSPConfig 7f7463a1d2a709ea
// Rust: Go's `(config, provided, err)`; `Ok((config, provided))` or the decoding error.
pub(crate) fn parse_lsp_config(
    value: Option<&str>,
) -> Result<(Option<Arc<EffectPluginOptions>>, bool), String> {
    let Some(value) = value else {
        return Ok((None, false));
    };
    let decoded = decode_any(value)?;
    let mut config: Vec<(String, JsonAny)> = Vec::new();
    if let JsonAny::Object(object) = decoded {
        config.extend(object);
    }
    map_set(&mut config, "name", JsonAny::String(EFFECT_PLUGIN_NAME.to_string()));
    Ok((
        ets_etscore::parse_from_plugins(&JsonAny::Array(vec![JsonAny::Object(config)]))
            .map(Arc::new),
        true,
    ))
}

/// Rust-only: Go's `versionRecord` (local to `collect`).
#[derive(Clone)]
struct VersionRecord {
    detected: String,
    supported: String,
}

// go: etsdiagnostics/diagnostics.go:160 collect 4427762a04a71d74
// Rust: Go's `(diagnostics, files, summary, error)`; Go's `files` is nil (omitted) without `ListFiles`, an empty
// list here (also omitted). The progress line pads by characters, as Go's `%60s` pads by runes.
pub(crate) fn collect(
    ctx: &Context,
    req: &Request,
    r#override: Option<&Arc<EffectPluginOptions>>,
    override_provided: bool,
    stderr: &mut dyn Write,
) -> Result<(Vec<FormattedDiagnostic>, Vec<FileEffectVersion>, Summary), String> {
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(osvfs::fs()));
    let session = project::new_session(&project::SessionInit {
        background_ctx: ctx.clone(),
        options: Arc::new(project::SessionOptions {
            current_directory: req.cwd.clone(),
            default_library_path: bundled::lib_path(),
            position_encoding: ets_shim::lsproto::PositionEncodingKind::UTF8,
            ..Default::default()
        }),
        fs: Arc::clone(&fs),
        client: None,
        logger: None,
        npm_executor: None,
        parse_cache: None,
    });
    let _close = CloseSession(&session);

    let mut targets: Vec<String> = Vec::new();
    let mut seen_targets: FxHashSet<tspath::Path> = FxHashSet::default();
    let use_case_sensitive_file_names = fs.use_case_sensitive_file_names();
    let mut add_target = |file_name: String| {
        let path = tspath::to_path(
            &tspath::get_normalized_absolute_path(&file_name, &req.cwd),
            "",
            use_case_sensitive_file_names,
        );
        if !seen_targets.insert(path) {
            return;
        }
        targets.push(file_name);
    };

    if !req.project.is_empty() {
        let mut project_name = tspath::get_normalized_absolute_path(&req.project, &req.cwd);
        if fs.directory_exists(&project_name) {
            project_name = tspath::get_normalized_absolute_path("tsconfig.json", &project_name);
        }
        let mut open_projects = Set::default();
        open_projects.add(project_name);
        update_session(
            ctx,
            &session,
            APISnapshotRequest { open_projects: Some(open_projects), ..Default::default() },
        )?;

        session.with_snapshot_loading_project_tree(ctx, None, |snapshot| {
            for configured_project in snapshot.project_collection.configured_projects() {
                let Some(command_line) = configured_project.command_line else {
                    continue;
                };
                for file_name in command_line.file_names() {
                    add_target(file_name.clone());
                }
            }
        });
    }

    if !req.file.is_empty() {
        let file_name = tspath::get_normalized_absolute_path(&req.file, &req.cwd);
        let request =
            new_open_file_snapshot_request(&file_name, &req.cwd, use_case_sensitive_file_names);
        update_session(ctx, &session, request)?;
        add_target(file_name);
    }

    let mut result_summary = Summary { total_files: targets.len() as i64, ..Default::default() };
    if targets.is_empty() {
        return Err(NO_FILES_MESSAGE.to_string());
    }

    let severity_filter = parse_severity_filter(&req.severity);
    let mut results: Vec<FormattedDiagnostic> = Vec::new();
    let mut files: Vec<FileEffectVersion> = Vec::new();
    let mut version_cache: FxHashMap<usize, VersionRecord> = FxHashMap::default();
    if req.list_files {
        files = Vec::with_capacity(targets.len());
    }
    let ctx = context::with_checker_lifetime(ctx, CheckerLifetime::Diagnostics);
    if req.progress {
        let _ = writeln!(stderr, "Starting diagnostics for {} files...", targets.len());
    }

    let mut collect_err: Option<String> = None;
    session.with_snapshot_loading_project_tree(&ctx, None, |snapshot: &Arc<Snapshot>| {
        for (index, file_name) in targets.iter().enumerate() {
            if req.progress {
                let _ = write!(
                    stderr,
                    "[{}/{}] {:>60}\r",
                    index + 1,
                    targets.len(),
                    truncate_left(file_name, 60)
                );
            }
            let uri = ets_shim::ls::lsconv::file_name_to_document_uri(file_name);
            let Some(configured_project) = snapshot.get_default_project(&uri) else {
                continue;
            };
            let Some(program) = configured_project.get_program() else {
                continue;
            };
            let Some(source_file) = program.get_source_file(file_name) else {
                continue;
            };
            if override_provided {
                program.options().effect.set(r#override.cloned());
            }
            if program.options().effect.is_none() {
                if !override_provided {
                    result_summary.files_without_plugin += 1;
                }
                continue;
            }

            if req.list_files {
                let key = Arc::as_ptr(configured_project.arc()) as usize;
                let record = if let Some(record) = version_cache.get(&key) {
                    record.clone()
                } else {
                    let mut c = program.get_type_checker(&ctx);
                    let mut tp = ets_typeparser::new_type_parser(program, &mut c);
                    let record = VersionRecord {
                        detected: tp.detect_effect_version().to_string(),
                        supported: tp.supported_effect_version().to_string(),
                    };
                    drop(tp);
                    drop(c);
                    version_cache.insert(key, record.clone());
                    record
                };
                files.push(FileEffectVersion {
                    file: file_name.clone(),
                    detected_effect: record.detected,
                    supported_effect: record.supported,
                });
            }

            for diagnostic in program.get_semantic_diagnostics(&ctx, Some(source_file)) {
                if !ets_rule::is_effect_code(diagnostic.code()) {
                    continue;
                }
                let formatted = format_diagnostic(diagnostic);
                if let Some(severity_filter) = &severity_filter {
                    if !severity_filter.contains(&formatted.severity) {
                        continue;
                    }
                }
                match formatted.severity {
                    Severity::Error => result_summary.errors += 1,
                    Severity::Warning => result_summary.warnings += 1,
                    Severity::Message => result_summary.messages += 1,
                }
                results.push(formatted);
            }
            result_summary.files_checked += 1;
            if let Some(err) = ctx.err() {
                collect_err = Some(err.to_string());
                return;
            }
        }
    });
    if req.progress {
        let _ = writeln!(stderr);
    }
    if let Some(collect_err) = collect_err {
        return Err(collect_err);
    }
    Ok((results, files, result_summary))
}

// go: etsdiagnostics/diagnostics.go:319 updateSession 54120a1a21956608
// Rust: Go's `session.APIUpdate` is tsrs's `Session::api_update`; Go's `DerefSnapshot` (shim integration) is
// `Snapshot::deref`.
pub(crate) fn update_session(
    ctx: &Context,
    session: &Session,
    request: APISnapshotRequest,
) -> Result<(), String> {
    match session.api_update(ctx, &FileChangeSummary::default(), Some(Arc::new(request))) {
        Ok(snapshot) => {
            snapshot.deref();
            Ok(())
        }
        Err(err) => Err(err.to_string()),
    }
}

// go: etsdiagnostics/diagnostics.go:327 parseSeverityFilter 22cd02581e78cc45
pub(crate) fn parse_severity_filter(value: &str) -> Option<FxHashSet<Severity>> {
    if value.is_empty() {
        return None;
    }
    let mut result = FxHashSet::default();
    for item in value.split(',') {
        let level =
            ets_shim::core::stringutil::go_strings_to_lower(ets_go::strings::trim_space(item));
        if let Some(level) = Severity::from_str(&level) {
            result.insert(level);
        }
    }
    if result.is_empty() {
        return None;
    }
    Some(result)
}

// go: etsdiagnostics/diagnostics.go:344 formatDiagnostic 71b2a852884c1dec
pub(crate) fn format_diagnostic(diagnostic: P<Diagnostic>) -> FormattedDiagnostic {
    let mut name = ets_rule::code_to_rule_name(&ets_rules::ALL, diagnostic.code()).to_string();
    if name.is_empty() {
        name = format!("effect({})", diagnostic.code());
    }
    let mut related = Vec::new();
    for &info in diagnostic.related_information() {
        if info.file().is_none() {
            continue;
        }
        related.push(RelatedInformation {
            location: format_location(info),
            code: info.code(),
            message: format_message(info, &name),
        });
    }
    FormattedDiagnostic {
        location: format_location(diagnostic),
        severity: category_to_severity(diagnostic.category()),
        code: diagnostic.code(),
        message: format_message(diagnostic, &name),
        name,
        related_information: related,
    }
}

// go: etsdiagnostics/diagnostics.go:370 formatLocation a3e0a4396b1f5905
// Rust: the diagnostics formatted here have a file and a range (Go reads them without checks).
pub(crate) fn format_location(diagnostic: P<Diagnostic>) -> Location {
    let file = diagnostic.file().expect("diagnostic file");
    let pos = diagnostic.pos().unwrap_or_default();
    let end_pos = diagnostic.end().unwrap_or_default();
    let start = file.get_position_map().utf8_to_utf16(pos) as i64;
    let end = file.get_position_map().utf8_to_utf16(end_pos) as i64;
    let (line, column) =
        ets_shim::scanner::get_ecma_line_and_utf16_character_of_position(&*file, pos);
    let (end_line, end_column) =
        ets_shim::scanner::get_ecma_line_and_utf16_character_of_position(&*file, end_pos);
    Location {
        file: file.file_name().to_string(),
        start,
        length: end - start,
        line: line as i64 + 1,
        column: column as i64 + 1,
        end_line: end_line as i64 + 1,
        end_column: end_column as i64 + 1,
        source: file.text(),
    }
}

// go: etsdiagnostics/diagnostics.go:388 formatMessage e799b65967722966
pub(crate) fn format_message(diagnostic: P<Diagnostic>, name: &str) -> String {
    let message = flatten_message(diagnostic, 0);
    let suffix = format!(" effect({name})");
    match message.strip_suffix(&suffix) {
        Some(trimmed) => trimmed.to_string(),
        None => message,
    }
}

// go: etsdiagnostics/diagnostics.go:392 flattenMessage cdcf854cda7fe046
pub(crate) fn flatten_message(diagnostic: P<Diagnostic>, level: usize) -> String {
    let mut output = String::new();
    output.push_str(&diagnostic.localize());
    for &child in diagnostic.message_chain() {
        output.push('\n');
        output.push_str(&"  ".repeat(level + 1));
        output.push_str(&flatten_message(child, level + 1));
    }
    output
}

// go: etsdiagnostics/diagnostics.go:403 categoryToSeverity 5a4218fd6f9e94bf
pub(crate) fn category_to_severity(category: tsdiag::Category) -> Severity {
    match category {
        tsdiag::Category::Error => Severity::Error,
        tsdiag::Category::Warning => Severity::Warning,
        _ => Severity::Message,
    }
}

// go: etsdiagnostics/diagnostics.go:414 writeOutput 1e544408217c0451
pub(crate) fn write_output(
    output: &mut dyn Write,
    format: &str,
    diagnostics: &[FormattedDiagnostic],
    files: &[FileEffectVersion],
    result_summary: Summary,
) -> std::io::Result<()> {
    match format {
        "json" => {
            return output.write_all(
                encode_json_output(&JsonOutput { diagnostics, files, summary: result_summary })
                    .as_bytes(),
            );
        }
        "github-actions" => {
            for diagnostic in diagnostics {
                let mut command = diagnostic.severity.as_str();
                if diagnostic.severity == Severity::Message {
                    command = "notice";
                }
                let message = diagnostic
                    .message
                    .replace('%', "%25")
                    .replace('\r', "%0D")
                    .replace('\n', "%0A");
                let _ = writeln!(
                    output,
                    "::{command} file={},line={},col={},endLine={},endColumn={},title={}::{message}",
                    diagnostic.location.file,
                    diagnostic.location.line,
                    diagnostic.location.column,
                    diagnostic.location.end_line,
                    diagnostic.location.end_column,
                    diagnostic.name
                );
            }
        }
        "text" => {
            for diagnostic in diagnostics {
                let _ = writeln!(
                    output,
                    "{}({},{}): {} {}: {}",
                    diagnostic.location.file,
                    diagnostic.location.line,
                    diagnostic.location.column,
                    diagnostic.severity.as_str(),
                    diagnostic_label(&diagnostic.name),
                    diagnostic.message
                );
            }
        }
        "pretty" => {
            for diagnostic in diagnostics {
                write_pretty_diagnostic(output, diagnostic);
            }
        }
        _ => {}
    }
    if !files.is_empty() {
        let _ = writeln!(output, "Effect version per file:");
        for file in files {
            let _ = writeln!(
                output,
                "  {}: detected={}, supported={}",
                file.file, file.detected_effect, file.supported_effect
            );
        }
    }
    let _ = writeln!(
        output,
        "Checked {} files out of {} files. ",
        result_summary.files_checked, result_summary.total_files
    );
    if result_summary.files_without_plugin > 0 {
        let _ = writeln!(
            output,
            "Skipped {} files because their tsconfig does not enable the {} plugin.",
            result_summary.files_without_plugin, EFFECT_PLUGIN_NAME
        );
    }
    writeln!(
        output,
        "{} errors, {} warnings and {} messages.",
        result_summary.errors, result_summary.warnings, result_summary.messages
    )
}

// go: etsdiagnostics/diagnostics.go:456 writePrettyDiagnostic a47924a2a8160649
pub(crate) fn write_pretty_diagnostic(output: &mut dyn Write, diagnostic: &FormattedDiagnostic) {
    let color = match diagnostic.severity {
        Severity::Error => "\x1b[91m",
        Severity::Warning => "\x1b[93m",
        Severity::Message => CYAN_COLOR,
    };
    let _ = writeln!(
        output,
        "{color}{}:{}:{} - {} {}:{RESET_COLOR} {}",
        diagnostic.location.file,
        diagnostic.location.line,
        diagnostic.location.column,
        diagnostic.severity.as_str(),
        diagnostic_label(&diagnostic.name),
        diagnostic.message
    );
    let _ = writeln!(output);
    write_code_snippet(output, &diagnostic.location, "", color);
    let _ = writeln!(output);
    for related in &diagnostic.related_information {
        let _ = writeln!(
            output,
            "  {CYAN_COLOR}{}:{}:{}{RESET_COLOR} - {}",
            related.location.file, related.location.line, related.location.column, related.message
        );
        write_code_snippet(output, &related.location, "    ", CYAN_COLOR);
        let _ = writeln!(output);
    }
}

// go: etsdiagnostics/diagnostics.go:478 cyanColor 33ab20a728b75206
pub(crate) const CYAN_COLOR: &str = "\x1b[96m";
// go: etsdiagnostics/diagnostics.go:478 cyanColor 33ab20a728b75206
pub(crate) const RESET_COLOR: &str = "\x1b[0m";

// go: etsdiagnostics/diagnostics.go:483 writeCodeSnippet 0825c57aafc79dcf
pub(crate) fn write_code_snippet(
    output: &mut dyn Write,
    loc: &Location,
    indent: &str,
    color: &str,
) {
    let lines: Vec<&str> = loc.source.split('\n').collect();
    if loc.line <= 0 || loc.line > lines.len() as i64 {
        return;
    }
    let line = lines[(loc.line - 1) as usize];
    let line = line.strip_suffix('\r').unwrap_or(line);
    let gutter = loc.line.to_string() + " ";
    let _ = writeln!(
        output,
        "{indent}{gutter}{line}\n{indent}{}{color}{}{RESET_COLOR}",
        " ".repeat(gutter.len()),
        " ".repeat((loc.column - 1).max(0) as usize)
            + &"~".repeat((loc.end_column - loc.column).max(1) as usize)
    );
}

// go: etsdiagnostics/diagnostics.go:494 diagnosticLabel 3c2c1c88e6a2c936
pub(crate) fn diagnostic_label(name: &str) -> String {
    if name.starts_with("effect(") {
        return name.to_string();
    }
    "effect(".to_string() + name + ")"
}

// go: etsdiagnostics/diagnostics.go:501 truncateLeft 40e09d4acaabb6e5
// Rust: Go slices at a byte offset, which may split a character; the cut is moved to the next character boundary
// (the progress line is cosmetic).
pub(crate) fn truncate_left(value: &str, width: usize) -> &str {
    if value.len() <= width {
        return value;
    }
    let mut start = value.len() - width;
    while !value.is_char_boundary(start) {
        start += 1;
    }
    &value[start..]
}

/// Rust-only: Go's `defer session.Close()`.
struct CloseSession<'a>(&'a Arc<Session>);

impl Drop for CloseSession<'_> {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// Rust-only: the shim integration helper `project.NewOpenFileSnapshotRequest(fileName, currentDirectory,
/// useCaseSensitiveFileNames)` (`_tools/gen_shims/providers/typescript/project/integration.go`): a request that
/// opens one file, keyed by its canonical path.
pub(crate) fn new_open_file_snapshot_request(
    file_name: &str,
    current_directory: &str,
    use_case_sensitive_file_names: bool,
) -> APISnapshotRequest {
    let file = tspath::get_normalized_absolute_path(file_name, current_directory);
    let mut open_files = FxHashMap::default();
    open_files.insert(tspath::to_path(&file, "", use_case_sensitive_file_names), file);
    APISnapshotRequest { open_files: Some(open_files), ..Default::default() }
}

// ---------------------------------------------------------------------------------------------------------------
// Rust-only: Go's encoding/json, as far as this package uses it.

/// Rust-only: a decoded JSON value as Go's `encoding/json` decodes into `any` (numbers are `float64`, objects are
/// `map[string]any`; a duplicate key keeps its last value).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum JsonAny {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonAny>),
    Object(Vec<(String, JsonAny)>),
}

impl JsonAny {
    fn from_value(value: serde_json::Value) -> JsonAny {
        match value {
            serde_json::Value::Null => JsonAny::Null,
            serde_json::Value::Bool(b) => JsonAny::Bool(b),
            serde_json::Value::Number(n) => JsonAny::Number(n.as_f64().unwrap_or(f64::NAN)),
            serde_json::Value::String(s) => JsonAny::String(s),
            serde_json::Value::Array(a) => {
                JsonAny::Array(a.into_iter().map(JsonAny::from_value).collect())
            }
            serde_json::Value::Object(o) => {
                let mut entries = Vec::new();
                for (k, v) in o {
                    map_set(&mut entries, &k, JsonAny::from_value(v));
                }
                JsonAny::Object(entries)
            }
        }
    }

    fn kind_name(&self) -> &'static str {
        match self {
            JsonAny::Null => "null",
            JsonAny::Bool(_) => "bool",
            JsonAny::Number(_) => "number",
            JsonAny::String(_) => "string",
            JsonAny::Array(_) => "array",
            JsonAny::Object(_) => "object",
        }
    }
}

/// Rust-only: Go's `m[key] = value` on a `map[string]any`.
fn map_set(map: &mut Vec<(String, JsonAny)>, key: &str, value: JsonAny) {
    if let Some(entry) = map.iter_mut().find(|(k, _)| k == key) {
        entry.1 = value;
    } else {
        map.push((key.to_string(), value));
    }
}

impl AnyValue for JsonAny {
    fn is_nil(&self) -> bool {
        matches!(self, JsonAny::Null)
    }
    fn as_bool(&self) -> Option<bool> {
        match self {
            JsonAny::Bool(b) => Some(*b),
            _ => None,
        }
    }
    fn as_float64(&self) -> Option<f64> {
        match self {
            JsonAny::Number(n) => Some(*n),
            _ => None,
        }
    }
    fn as_string(&self) -> Option<&str> {
        match self {
            JsonAny::String(s) => Some(s),
            _ => None,
        }
    }
    fn as_slice(&self) -> Option<&[Self]> {
        match self {
            JsonAny::Array(a) => Some(a),
            _ => None,
        }
    }
    fn as_string_any_map(&self) -> Option<Vec<(&str, &Self)>> {
        match self {
            JsonAny::Object(o) => Some(o.iter().map(|(k, v)| (k.as_str(), v)).collect()),
            _ => None,
        }
    }
}

/// Rust-only: `json.Unmarshal(data, &decoded)` with `decoded any`. The error text of a syntax error is the decoder's
/// (porting/DIVERGENCES.md); callers print only that an error occurred.
pub(crate) fn decode_any(data: &str) -> Result<JsonAny, String> {
    serde_json::from_str::<serde_json::Value>(data)
        .map(JsonAny::from_value)
        .map_err(|err| err.to_string())
}

/// Rust-only: `json.Unmarshal([]byte(data), &req)` into `request`: keys match the `json:` names exactly or, failing
/// that, case-insensitively (Go's rule); unknown keys are ignored; `null` leaves a field unchanged; a value of the
/// wrong type is Go's `UnmarshalTypeError`. Go keeps decoding after a type error and reports the first one.
pub(crate) fn decode_request(data: &str) -> Result<Request, String> {
    let decoded = decode_any(data)?;
    let mut req = Request::default();
    let object = match decoded {
        JsonAny::Null => return Ok(req),
        JsonAny::Object(object) => object,
        other => {
            return Err(format!(
                "json: cannot unmarshal {} into Go value of type etsdiagnostics.request",
                other.kind_name()
            ));
        }
    };
    const FIELDS: [&str; 9] = [
        "cwd",
        "file",
        "project",
        "format",
        "strict",
        "severity",
        "progress",
        "lspconfig",
        "listFiles",
    ];
    let mut first_error: Option<String> = None;
    for (key, value) in object {
        let field = FIELDS
            .iter()
            .find(|name| **name == key)
            .or_else(|| FIELDS.iter().find(|name| name.eq_ignore_ascii_case(&key)));
        let Some(&field) = field else {
            continue;
        };
        if value == JsonAny::Null {
            if field == "lspconfig" {
                req.lsp_config = None;
            }
            continue;
        }
        let is_bool = matches!(field, "strict" | "progress" | "listFiles");
        let type_name = if is_bool { "bool" } else { "string" };
        let ok = match (&value, is_bool) {
            (JsonAny::Bool(b), true) => {
                match field {
                    "strict" => req.strict = *b,
                    "progress" => req.progress = *b,
                    _ => req.list_files = *b,
                }
                true
            }
            (JsonAny::String(s), false) => {
                let s = s.clone();
                match field {
                    "cwd" => req.cwd = s,
                    "file" => req.file = s,
                    "project" => req.project = s,
                    "format" => req.format = s,
                    "severity" => req.severity = s,
                    _ => req.lsp_config = Some(s),
                }
                true
            }
            _ => false,
        };
        if !ok && first_error.is_none() {
            let go_type = if field == "lspconfig" { "*string" } else { type_name };
            first_error = Some(format!(
                "json: cannot unmarshal {} into Go struct field request.{field} of type {go_type}",
                value.kind_name()
            ));
        }
    }
    match first_error {
        Some(err) => Err(err),
        None => Ok(req),
    }
}

/// Rust-only: `json.NewEncoder(output)` with `SetIndent("", "  ")`, then `Encode(jsonOutput{...})`.
pub(crate) fn encode_json_output(value: &JsonOutput<'_>) -> String {
    let mut fields: Vec<(&str, String)> = Vec::new();
    fields.push((
        "diagnostics",
        json_array(
            &value.diagnostics.iter().map(encode_formatted_diagnostic).collect::<Vec<_>>(),
            1,
        ),
    ));
    if !value.files.is_empty() {
        fields.push((
            "files",
            json_array(
                &value
                    .files
                    .iter()
                    .map(|file| {
                        json_object(
                            &[
                                ("file", json_string(&file.file)),
                                ("detectedEffect", json_string(&file.detected_effect)),
                                ("supportedEffect", json_string(&file.supported_effect)),
                            ],
                            2,
                        )
                    })
                    .collect::<Vec<_>>(),
                1,
            ),
        ));
    }
    let s = value.summary;
    fields.push((
        "summary",
        json_object(
            &[
                ("filesChecked", s.files_checked.to_string()),
                ("filesWithoutPlugin", s.files_without_plugin.to_string()),
                ("totalFiles", s.total_files.to_string()),
                ("errors", s.errors.to_string()),
                ("warnings", s.warnings.to_string()),
                ("messages", s.messages.to_string()),
            ],
            1,
        ),
    ));
    json_object(&fields, 0) + "\n"
}

fn encode_location(loc: &Location) -> Vec<(&'static str, String)> {
    vec![
        ("file", json_string(&loc.file)),
        ("start", loc.start.to_string()),
        ("length", loc.length.to_string()),
        ("line", loc.line.to_string()),
        ("column", loc.column.to_string()),
        ("endLine", loc.end_line.to_string()),
        ("endColumn", loc.end_column.to_string()),
    ]
}

fn encode_formatted_diagnostic(d: &FormattedDiagnostic) -> String {
    let mut fields = encode_location(&d.location);
    fields.push(("severity", json_string(d.severity.as_str())));
    fields.push(("code", d.code.to_string()));
    fields.push(("name", json_string(&d.name)));
    fields.push(("message", json_string(&d.message)));
    if !d.related_information.is_empty() {
        fields.push((
            "relatedInformation",
            json_array(
                &d.related_information
                    .iter()
                    .map(|r| {
                        let mut fields = encode_location(&r.location);
                        fields.push(("code", r.code.to_string()));
                        fields.push(("message", json_string(&r.message)));
                        json_object(&fields, 4)
                    })
                    .collect::<Vec<_>>(),
                3,
            ),
        ));
    }
    json_object(&fields, 2)
}

/// An indented object whose opening brace is at nesting `depth` (Go's `json.Indent` layout).
fn json_object(fields: &[(&str, String)], depth: usize) -> String {
    if fields.is_empty() {
        return "{}".to_string();
    }
    let inner = "  ".repeat(depth + 1);
    let mut out = String::from("{\n");
    for (i, (key, value)) in fields.iter().enumerate() {
        out.push_str(&inner);
        out.push_str(&json_string(key));
        out.push_str(": ");
        out.push_str(value);
        if i + 1 < fields.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&"  ".repeat(depth));
    out.push('}');
    out
}

/// An indented array of already-encoded elements whose opening bracket is at nesting `depth`.
fn json_array(elements: &[String], depth: usize) -> String {
    if elements.is_empty() {
        return "[]".to_string();
    }
    let inner = "  ".repeat(depth + 1);
    let mut out = String::from("[\n");
    for (i, element) in elements.iter().enumerate() {
        out.push_str(&inner);
        out.push_str(element);
        if i + 1 < elements.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&"  ".repeat(depth));
    out.push(']');
    out
}

/// Go's encoding/json string encoding with HTML escaping (the encoder's default).
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' | '>' | '&' | '\u{2028}' | '\u{2029}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Go's `%q` of a string, for the format names in error messages (`strconv.Quote` on printable ASCII and the
/// common escapes).
fn go_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
