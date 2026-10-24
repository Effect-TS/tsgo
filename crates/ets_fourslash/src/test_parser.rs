//! Rust-only: port of typescript-go's `fourslash/test_parser.go` (the fourslash test-data parser) and of the part of
//! `testrunner/test_case_parser.go` it calls (`ParseTestFilesAndSymlinksWithOptions`). tsrs ports the latter only
//! inside its `tsrs_testrunner` binary, so it is ported here again. The Effect runners feed whole test files through
//! this parser, as Go's do: marker syntax in a case (`/*name*/`, `[|`, `|]`, `{| ... |}`) is stripped from the file
//! contents exactly as in Go.

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};

use ets_shim::core::stringutil;
use ets_shim::core::tspath;
use ets_shim::core::{TextPos, TextRange};
use ets_shim::ls::lsconv;
use ets_shim::lsproto;
use regex::Regex;
use rustc_hash::FxHashMap;

use crate::*;

// Rust-only: test_case_parser.go:18 lineDelimiter.
static LINE_DELIMITER: LazyLock<Regex> = LazyLock::new(|| Regex::new("\r?\n").unwrap());

// Rust-only: test_case_parser.go:41 optionRegex. Go's RE2 `\s` and `\w` are ASCII classes; they are spelled out.
static OPTION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^/{2}[\t\n\x0C\r ]*@([0-9A-Za-z_]+)[\t\n\x0C\r ]*:[\t\n\x0C\r ]*([^\r\n]*)")
        .unwrap()
});

// Rust-only: test_case_parser.go:44 linkRegex.
static LINK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^/{2}[\t\n\x0C\r ]*@link[\t\n\x0C\r ]*:[\t\n\x0C\r ]*([^\r\n]*)[\t\n\x0C\r ]*->[\t\n\x0C\r ]*([^\r\n]*)",
    )
    .unwrap()
});

// Rust-only: test_case_parser.go:47 fourslashDirectives.
const FOURSLASH_DIRECTIVES: &[&str] = &["emitthisfile", "noopen"];

/// Rust-only: `test_case_parser.go` `ParseTestFilesOptions`.
#[derive(Clone, Copy, Default)]
pub struct ParseTestFilesOptions {
    // If true, allows test content to appear before the first @Filename directive.
    // In this case, an implicit first file is created using the fileName parameter.
    // This matches the behavior of the TypeScript fourslash test harness.
    pub allow_implicit_first_file: bool,
}

/// Rust-only: the results of `parse_test_files_and_symlinks_with_options`.
///
/// The units, the symlinks, the current directory and the global options (Go's `map[string]string`s are
/// `BTreeMap`s; nothing observable iterates them).
pub type ParsedTestFiles<T> = (Vec<T>, BTreeMap<String, String>, String, BTreeMap<String, String>);

/// Rust-only: `test_case_parser.go` `ParseTestFilesAndSymlinksWithOptions`.
pub fn parse_test_files_and_symlinks_with_options<T>(
    code: &str,
    file_name: &str,
    mut parse_file: impl FnMut(&str, &str, &BTreeMap<String, String>) -> Result<T, String>,
    options: ParseTestFilesOptions,
) -> Result<ParsedTestFiles<T>, String> {
    // List of all the subfiles we've parsed out
    let mut test_units: Vec<T> = Vec::new();

    // Stuff related to the subfile we're parsing
    let mut current_file_content = String::new();
    let mut current_file_name = String::new();
    let mut seen_content_line = false;
    let mut has_seen_file = false;
    if options.allow_implicit_first_file {
        // For fourslash tests, initialize currentFileName to the fileName parameter
        // so content before the first @Filename directive goes into an implicit first file
        current_file_name = file_name.to_string();
    }
    let mut current_directory = String::new();
    let mut current_file_options: BTreeMap<String, String> = BTreeMap::new();
    let mut symlinks: BTreeMap<String, String> = BTreeMap::new();
    let mut global_options: BTreeMap<String, String> = BTreeMap::new();

    for line in LINE_DELIMITER.split(code) {
        if parse_symlink_from_test(line, &mut symlinks) {
            continue;
        }
        if let Some(test_meta_data) = OPTION_REGEX.captures(line) {
            // Comment line, check for global/file @options and record them
            let meta_data_name = ets_go::strings::to_lower(&test_meta_data[1]);
            let meta_data_value = ets_go::strings::trim_space(&test_meta_data[2]).to_string();
            if meta_data_name == "currentdirectory" {
                current_directory.clone_from(&meta_data_value);
            }
            if meta_data_name != "filename" {
                if meta_data_name == "symlink" && !current_file_name.is_empty() {
                    for link in meta_data_value.split(',') {
                        let link = ets_go::strings::trim_space(link);
                        if !link.is_empty() {
                            symlinks.insert(link.to_string(), current_file_name.clone());
                        }
                    }
                } else if FOURSLASH_DIRECTIVES.contains(&meta_data_name.as_str()) {
                    // File-specific option
                    current_file_options.insert(meta_data_name, meta_data_value);
                } else {
                    // Global option
                    global_options.insert(meta_data_name, meta_data_value);
                }
                continue;
            }

            // New metadata statement after having collected some code to go with the previous metadata
            if current_file_name.is_empty() {
                // First metadata marker in the file
                let has_content_before_first_filename = !current_file_content.is_empty()
                    && ets_shim::scanner::skip_trivia(&current_file_content, 0) as usize
                        != current_file_content.len();
                assert!(
                    !has_content_before_first_filename || options.allow_implicit_first_file,
                    "Non-comment test content appears before the first '// @Filename' directive"
                );

                // Reset for the new file
                current_file_content.clear();
                seen_content_line = false;
                current_file_name = ets_go::strings::trim_space(&test_meta_data[2]).to_string();
                current_file_options = BTreeMap::new();
            } else {
                // Store result file - always save for regular tests, but skip empty implicit first file for fourslash
                let should_save_file = !options.allow_implicit_first_file
                    || !current_file_content.is_empty()
                    || has_seen_file;
                if should_save_file {
                    has_seen_file = true;
                    test_units.push(parse_file(
                        &current_file_name,
                        &current_file_content,
                        &current_file_options,
                    )?);
                }

                // Reset local data
                current_file_content.clear();
                seen_content_line = false;
                current_file_name = meta_data_value;
                current_file_options = BTreeMap::new();
            }
        } else {
            // Subfile content line
            // Append to the current subfile content, inserting a newline if needed
            // For fourslash tests, use seenContentLine to preserve leading blank lines
            // (matching TS fourslash's //// content markers). For compiler tests, use
            // Len() != 0 which drops leading blanks (matching TS's harness behavior).
            if options.allow_implicit_first_file {
                if seen_content_line {
                    current_file_content.push('\n');
                }
                seen_content_line = true;
            } else if !current_file_content.is_empty() {
                current_file_content.push('\n');
            }
            current_file_content.push_str(line);
        }
    }

    // normalize the fileName for the single file case
    if test_units.is_empty() && current_file_name.is_empty() {
        current_file_name = tspath::get_base_file_name(file_name);
    }

    // EOF, push whatever remains
    test_units.push(parse_file(&current_file_name, &current_file_content, &current_file_options)?);

    Ok((test_units, symlinks, current_directory, global_options))
}

/// Rust-only: `test_case_parser.go` `parseSymlinkFromTest`.
fn parse_symlink_from_test(line: &str, symlinks: &mut BTreeMap<String, String>) -> bool {
    let Some(link_meta_data) = LINK_REGEX.captures(line) else {
        return false;
    };
    symlinks.insert(
        ets_go::strings::trim_space(&link_meta_data[2]).to_string(),
        ets_go::strings::trim_space(&link_meta_data[1]).to_string(),
    );
    true
}

/// Rust-only: `test_parser.go` `RangeMarker`: text in a range marked with `[|` and `|]`. `marker` is the index (in
/// `TestData::markers`) of the marker at the range's start (Go: a pointer to it).
#[derive(Clone, Debug, Default)]
pub struct RangeMarker {
    pub(crate) file_name: String,
    pub range: TextRange,
    pub ls_range: lsproto::Range,
    pub marker: Option<usize>,
}

impl RangeMarker {
    /// Rust-only: `test_parser.go` `RangeMarker.FileName`.
    pub fn file_name(&self) -> &str {
        &self.file_name
    }
}

/// Rust-only: `test_parser.go` `Marker`. Object-marker data (Go's decoded `Data`) is kept as its JSON text.
#[derive(Clone, Debug)]
pub struct Marker {
    pub(crate) file_name: String,
    pub position: i64,
    pub ls_position: lsproto::Position,
    // `None` for anonymous markers such as `{| "foo": "bar" |}`
    pub name: Option<String>,
    pub data: Option<String>,
}

impl Marker {
    /// Rust-only: `test_parser.go` `Marker.FileName`.
    pub fn file_name(&self) -> &str {
        &self.file_name
    }
}

/// Rust-only: `test_parser.go` `TestData`. `marker_positions` maps a name to its index in `markers`.
#[derive(Clone, Debug, Default)]
pub struct TestData {
    pub files: Vec<TestFileInfo>,
    pub marker_positions: FxHashMap<String, usize>,
    pub markers: Vec<Marker>,
    pub symlinks: BTreeMap<String, String>,
    pub global_options: BTreeMap<String, String>,
    pub ranges: Vec<RangeMarker>,
}

impl TestData {
    /// Rust-only: `test_parser.go` `TestData.isStateBaseliningEnabled`.
    pub(crate) fn is_state_baselining_enabled(&self) -> bool {
        is_state_baselining_enabled(&self.global_options)
    }
}

/// Rust-only: `test_parser.go` `testFileWithMarkers`.
struct TestFileWithMarkers {
    file: TestFileInfo,
    markers: Vec<Marker>,
    ranges: Vec<RangeMarker>,
}

/// Rust-only: `test_parser.go` `isStateBaseliningEnabled`.
fn is_state_baselining_enabled(global_options: &BTreeMap<String, String>) -> bool {
    global_options.get("statebaseline").is_some_and(|v| v == "true")
}

/// Rust-only: `test_parser.go` `ParseTestData`. Go's `t.Fatalf` on a parse error is a panic with Go's message.
pub fn parse_test_data(contents: &str, file_name: &str) -> TestData {
    // List of all the subfiles we've parsed out
    let mut files = Vec::new();

    let mut marker_positions: FxHashMap<String, usize> = FxHashMap::default();
    let mut markers: Vec<Marker> = Vec::new();
    let mut ranges: Vec<RangeMarker> = Vec::new();

    let (files_with_marker, symlinks, _, global_options) =
        match parse_test_files_and_symlinks_with_options(
            contents,
            file_name,
            parse_file_content,
            ParseTestFilesOptions { allow_implicit_first_file: true },
        ) {
            Ok(result) => result,
            Err(e) => panic!("Error parsing fourslash data: {e}"),
        };

    let mut has_ts_config = false;
    for file in files_with_marker {
        has_ts_config = has_ts_config || is_config_file(&file.file.file_name);
        files.push(file.file);

        let base = markers.len();
        ranges.extend(
            file.ranges
                .into_iter()
                .map(|range| RangeMarker { marker: range.marker.map(|i| base + i), ..range }),
        );
        for marker in file.markers {
            match &marker.name {
                None => {
                    // The marker is an anonymous object marker, which does not need a name. Markers are only set into markerPositions if they have a name
                    assert!(
                        marker.data.is_some(),
                        "Marker at position {} is unnamed",
                        marker.position
                    );
                }
                Some(name) => {
                    if let Some(&existing) = marker_positions.get(name) {
                        panic!(
                            r#"Duplicate marker name: "{name}" at {} and {}"#,
                            marker.position, markers[existing].position
                        );
                    }
                    marker_positions.insert(name.clone(), markers.len());
                }
            }
            markers.push(marker);
        }
    }

    assert!(
        !(has_ts_config
            && has_unsupported_global_options_with_config(&global_options)
            && !is_state_baselining_enabled(&global_options)),
        "It is not allowed to use global options along with config files."
    );

    TestData { files, marker_positions, markers, symlinks, global_options, ranges }
}

/// Rust-only: `test_parser.go` `hasUnsupportedGlobalOptionsWithConfig`.
fn has_unsupported_global_options_with_config(global_options: &BTreeMap<String, String>) -> bool {
    global_options.keys().any(|option| {
        !matches!(
            ets_go::strings::to_lower(option).as_str(),
            "symlink" | "link" | "usecasesensitivefilenames"
        )
    })
}

/// Rust-only: `test_parser.go` `isConfigFile`.
pub(crate) fn is_config_file(file_name: &str) -> bool {
    let file_name = ets_go::strings::to_lower(file_name);
    file_name.ends_with("tsconfig.json") || file_name.ends_with("jsconfig.json")
}

/// Rust-only: `test_parser.go` `locationInformation`.
#[derive(Clone, Copy)]
struct LocationInformation {
    position: i64,
    source_position: usize,
    source_line: usize,
    source_column: usize,
}

/// Rust-only: `test_parser.go` `rangeLocationInformation`.
#[derive(Clone, Copy)]
struct RangeLocationInformation {
    location: LocationInformation,
    marker: Option<usize>,
}

/// Rust-only: `test_parser.go` `TestFileInfo`.
#[derive(Clone, Debug)]
pub struct TestFileInfo {
    pub(crate) file_name: String,
    // The contents of the file (with markers, etc stripped out)
    pub content: String,
    pub emit: bool,
    pub(crate) open: bool,
}

impl TestFileInfo {
    /// Rust-only: `test_parser.go` `TestFileInfo.FileName`.
    pub fn file_name(&self) -> &str {
        &self.file_name
    }
}

impl lsconv::Script for TestFileInfo {
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

// Rust-only: test_parser.go emitThisFileOption, noOpenFileOption.
const EMIT_THIS_FILE_OPTION: &str = "emitthisfile";
const NO_OPEN_FILE_OPTION: &str = "noopen";

/// Rust-only: `test_parser.go` `parserState`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParserState {
    None,
    InSlashStarMarker,
    InObjectMarker,
}

/// Rust-only: `test_parser.go` `parseFileContent`.
fn parse_file_content(
    file_name: &str,
    content: &str,
    file_options: &BTreeMap<String, String>,
) -> Result<TestFileWithMarkers, String> {
    parse_file_content_worker(&to_rooted_file_path(file_name), content, file_options)
}

/// Rust-only: Go's `tspath.ToRootedFilePath(fileName, rootDir)` (fourslash's root directory is `/`).
pub fn to_rooted_file_path(file_name: &str) -> String {
    tspath::get_normalized_absolute_path(file_name, ROOT_DIR)
}

/// Rust-only: Go's `content[lastNormalCharPosition:lastSafeCharIndex]` appends of `parseFileContentWorker`'s
/// `flush` closure (`None` is Go's `-1`: to the end).
fn flush(
    output: &mut String,
    content: &str,
    last_normal_char_position: usize,
    last_safe: Option<usize>,
) {
    match last_safe {
        Some(last_safe_char_index) => {
            output.push_str(&content[last_normal_char_position..last_safe_char_index]);
        }
        None => output.push_str(&content[last_normal_char_position..]),
    }
}

/// Rust-only: an offset as Go's `int` (contents are far below `i64::MAX`).
fn int(offset: usize) -> i64 {
    i64::try_from(offset).unwrap()
}

/// Rust-only: `test_parser.go` `parseFileContentWorker`. Positions are byte offsets, as Go's.
fn parse_file_content_worker(
    file_name: &str,
    content: &str,
    file_options: &BTreeMap<String, String>,
) -> Result<TestFileWithMarkers, String> {
    let content = chomp_leading_space(content);
    let content = content.as_str();

    // The file content (minus metacharacters) so far
    let mut output = String::new();

    let mut markers: Vec<Marker> = Vec::new();

    // A stack of the open range markers that are still unclosed
    let mut open_ranges: Vec<RangeLocationInformation> = Vec::new();
    // A list of closed ranges we've collected so far
    let mut range_markers: Vec<RangeMarker> = Vec::new();

    // The total number of metacharacters removed from the file (so far)
    let mut difference: i64 = 0;

    // One-based current position data
    let mut line: usize = 1;
    let mut column: usize = 1;

    // The current marker (or maybe multi-line comment?) we're parsing, possibly
    let mut open_marker: Option<LocationInformation> = None;

    // The latest position of the start of an unflushed plain text area
    let mut last_normal_char_position: usize = 0;

    let mut state = ParserState::None;
    let (mut previous_character, mut i) =
        content.chars().next().map_or(('\u{FFFD}', 0), |c| (c, c.len_utf8()));
    while i < content.len() {
        let current_character = content[i..].chars().next().unwrap();
        let size = current_character.len_utf8();
        match state {
            ParserState::None => {
                if previous_character == '[' && current_character == '|' {
                    // found a range start
                    open_ranges.push(RangeLocationInformation {
                        location: LocationInformation {
                            position: (int(i) - 1) - difference,
                            source_position: i - 1,
                            source_line: line,
                            source_column: column,
                        },
                        marker: None,
                    });
                    // copy all text up to marker position
                    flush(&mut output, content, last_normal_char_position, Some(i - 1));
                    last_normal_char_position = i + 1;
                    difference += 2;
                } else if previous_character == '|' && current_character == ']' {
                    // found a range end
                    let Some(range_start) = open_ranges.pop() else {
                        return Err(report_error(
                            file_name,
                            line,
                            column,
                            "Found range end with no matching start.",
                        ));
                    };

                    let closed_range = RangeMarker {
                        file_name: file_name.to_string(),
                        range: TextRange::new(
                            u32::try_from(range_start.location.position).unwrap(),
                            u32::try_from((int(i) - 1) - difference).unwrap(),
                        ),
                        ls_range: lsproto::Range::default(),
                        marker: range_start.marker,
                    };

                    range_markers.push(closed_range);

                    // copy all text up to range marker position
                    flush(&mut output, content, last_normal_char_position, Some(i - 1));
                    last_normal_char_position = i + 1;
                    difference += 2;
                } else if previous_character == '/'
                    && current_character == '*'
                    && (i + 1 >= content.len() || content.as_bytes()[i + 1] != b'/')
                {
                    // found a possible marker start
                    state = ParserState::InSlashStarMarker;
                    open_marker = Some(LocationInformation {
                        position: (int(i) - 1) - difference,
                        source_position: i - 1,
                        source_line: line,
                        source_column: column - 1,
                    });
                } else if previous_character == '{' && current_character == '|' {
                    // found an object marker start
                    state = ParserState::InObjectMarker;
                    open_marker = Some(LocationInformation {
                        position: (int(i) - 1) - difference,
                        source_position: i - 1,
                        source_line: line,
                        source_column: column,
                    });
                    flush(&mut output, content, last_normal_char_position, Some(i - 1));
                }
            }
            ParserState::InObjectMarker => {
                // Object markers are only ever terminated by |} and have no content restrictions
                if previous_character == '|' && current_character == '}' {
                    let om = open_marker.unwrap();
                    let object_marker_data =
                        ets_go::strings::trim_space(&content[om.source_position + 2..i - 1]);
                    let marker = get_object_marker(file_name, &om, object_marker_data)?;

                    if let Some(last) = open_ranges.last_mut() {
                        last.marker = Some(markers.len());
                    }
                    markers.push(marker);

                    // Set the current start to point to the end of the current marker to ignore its text
                    last_normal_char_position = i + 1;
                    difference += int(i + 1 - om.source_position);

                    // Reset the state
                    open_marker = None;
                    state = ParserState::None;
                }
            }
            ParserState::InSlashStarMarker => {
                if previous_character == '*' && current_character == '/' {
                    let om = open_marker.unwrap();
                    // Record the marker
                    // start + 2 to ignore the */, -1 on the end to ignore the * (/ is next)
                    let marker_name_text =
                        ets_go::strings::trim_space(&content[om.source_position + 2..i - 1])
                            .to_string();
                    let marker = Marker {
                        file_name: file_name.to_string(),
                        position: om.position,
                        ls_position: lsproto::Position::default(),
                        name: Some(marker_name_text),
                        data: None,
                    };
                    if let Some(last) = open_ranges.last_mut() {
                        last.marker = Some(markers.len());
                    }
                    markers.push(marker);

                    // Set the current start to point to the end of the current marker to ignore its text
                    flush(
                        &mut output,
                        content,
                        last_normal_char_position,
                        Some(om.source_position),
                    );
                    last_normal_char_position = i + 1;
                    difference += int(i + 1 - om.source_position);

                    // Reset the state
                    open_marker = None;
                    state = ParserState::None;
                } else if !(stringutil::is_digit(u32::from(current_character))
                    || stringutil::is_ascii_letter(u32::from(current_character))
                    || current_character == '$'
                    || current_character == '_')
                {
                    // Invalid marker character
                    if current_character == '*'
                        && i < content.len() - 1
                        && content.as_bytes()[i + 1] == b'/'
                    {
                        // The marker is about to be closed, ignore the 'invalid' char
                    } else {
                        // We've hit a non-valid marker character, so we were actually in a block comment
                        // Bail out the text we've gathered so far back into the output
                        flush(&mut output, content, last_normal_char_position, Some(i));
                        last_normal_char_position = i;
                        open_marker = None;
                        state = ParserState::None;
                    }
                }
            }
        }
        if current_character == '\n' && previous_character == '\r' {
            // Ignore trailing \n after \r
            i += size;
            continue;
        } else if current_character == '\n' || current_character == '\r' {
            line += 1;
            column = 1;
            i += size;
            continue;
        }
        column += 1;
        if i >= last_normal_char_position {
            previous_character = current_character;
        } else {
            previous_character = '\u{FFFD}'; // reset to avoid accidentally reusing marker delimiters as part of other markers
        }
        i += size;
    }

    // Add the remaining text
    flush(&mut output, content, last_normal_char_position, None);

    if let Some(open_range) = open_ranges.first() {
        return Err(report_error(
            file_name,
            open_range.location.source_line,
            open_range.location.source_column,
            "Unterminated range.",
        ));
    }

    if let Some(open_marker) = open_marker {
        return Err(report_error(
            file_name,
            open_marker.source_line,
            open_marker.source_column,
            "Unterminated marker.",
        ));
    }

    let output_string = output;
    // Set LS positions for markers
    let line_map = lsconv::compute_lsp_line_starts(&output_string);
    let converters = lsconv::new_converters(lsproto::PositionEncodingKind::UTF8, move |_: &str| {
        Some(Arc::clone(&line_map))
    });

    let emit = file_options.get(EMIT_THIS_FILE_OPTION).is_some_and(|v| v == "true");

    let test_file_info = TestFileInfo {
        file_name: file_name.to_string(),
        content: output_string,
        emit,
        open: file_options.get(NO_OPEN_FILE_OPTION).is_none_or(|v| v != "true"),
    };

    // Go's slices.SortStableFunc.
    range_markers.sort_by(|a, b| {
        let (a_pos, b_pos) = (text_pos(a.range.pos()), text_pos(b.range.pos()));
        if a_pos != b_pos {
            return a_pos.cmp(&b_pos);
        }
        text_pos(b.range.end()).cmp(&text_pos(a.range.end()))
    });

    for marker in &mut markers {
        marker.ls_position = position_to_line_and_character(
            &converters,
            &test_file_info,
            TextPos::from(u32::try_from(marker.position).unwrap()),
        );
    }
    for range_marker in &mut range_markers {
        range_marker.ls_range = lsproto::Range {
            start: position_to_line_and_character(
                &converters,
                &test_file_info,
                range_marker.range.pos(),
            ),
            end: position_to_line_and_character(
                &converters,
                &test_file_info,
                range_marker.range.end(),
            ),
        };
    }

    Ok(TestFileWithMarkers { file: test_file_info, markers, ranges: range_markers })
}

/// Rust-only: the source offset of a parsed position (a `TextPos` of a `TextRange` built from offsets).
pub(crate) fn text_pos(pos: TextPos) -> u32 {
    pos.expect_source("fourslash positions are source offsets")
}

/// Rust-only: `test_parser.go` `getObjectMarker`. Go decodes the marker text as JSON; the port checks it is a
/// non-empty JSON object (with tsrs's Go-compatible JSON decoder) and reads its `name`.
fn get_object_marker(
    file_name: &str,
    location: &LocationInformation,
    text: &str,
) -> Result<Marker, String> {
    use ets_shim::core::json::{self, Value};
    // Attempt to parse the marker value as JSON
    let v = json::unmarshal(&format!("{{ {text} }}")).map_err(|_| {
        report_error(
            file_name,
            location.source_line,
            location.source_column,
            &format!("Unable to parse marker text {text}"),
        )
    })?;
    let marker_value = match &v {
        Value::Object(marker_value) if !marker_value.is_empty() => marker_value,
        _ => {
            return Err(report_error(
                file_name,
                location.source_line,
                location.source_column,
                "Object markers can not be empty",
            ));
        }
    };

    let mut marker = Marker {
        file_name: file_name.to_string(),
        position: location.position,
        ls_position: lsproto::Position::default(),
        name: None,
        data: Some(text.to_string()),
    };

    // Object markers can be anonymous
    if let Some(Value::String(name)) = marker_value.get("name") {
        if !name.is_empty() {
            marker.name = Some(name.clone());
        }
    }

    Ok(marker)
}

/// Rust-only: `test_parser.go` `reportError`.
fn report_error(file_name: &str, line: usize, col: usize, message: &str) -> String {
    format!("{file_name} ({line},{col}): {message}")
}

/// Rust-only: `test_parser.go` `chompLeadingSpace`.
fn chomp_leading_space(content: &str) -> String {
    let lines: Vec<&str> = content.split('\n').collect();
    for line in &lines {
        if !line.is_empty() && !line.starts_with(' ') {
            return content.to_string();
        }
    }

    let result: Vec<&str> =
        lines.iter().map(|line| if line.is_empty() { "" } else { &line[1..] }).collect();
    result.join("\n")
}
