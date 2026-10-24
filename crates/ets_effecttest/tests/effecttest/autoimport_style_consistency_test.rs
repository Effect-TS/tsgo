// Rust: Go's blank imports of `etslshooks` and `etstesthooks`: each test registers them
// (`ets_effecttest::init_hooks`).

use std::sync::Arc;

use ets_fourslash::{ApplyCodeActionFromCompletionOptions, FourslashTest};
use ets_go::testing::T;
use ets_shim::core::Tristate;
use ets_shim::ls::lsconv::{self, LSPLineMap};
use ets_shim::ls::lsutil::UserPreferences;
use ets_shim::lsproto;
use libtest_mimic::Trial;

use crate::run_case;

/// Rust-only: the `&lsutil.UserPreferences{...}` every test of the file passes.
fn completion_preferences() -> UserPreferences {
    UserPreferences {
        include_completions_for_module_exports: Tristate::True,
        include_completions_for_import_statements: Tristate::True,
        ..Default::default()
    }
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:18 TestAutoImportEffectStyleConsistency_namespace
fn test_auto_import_effect_style_consistency_namespace(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["EFFECT"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
runPromiseExit/*completion*/();
// @Filename: /mainFix.ts
runPromiseExit/*fix*/();
"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    let preferences = completion_preferences();
    let completion = "completion";

    f.verify_apply_code_action_from_completion(
        t,
        completion,
        &ApplyCodeActionFromCompletionOptions {
            name: "runPromiseExit".to_string(),
            source: "effect/Effect".to_string(),
            description: "Add import from \"effect/Effect\"".to_string(),
            new_file_content: Some(
                r#"import * as Effect from "effect/Effect";

Effect.runPromiseExit();"#
                    .to_string(),
            ),
            user_preferences: Some(preferences.clone()),
            ..Default::default()
        },
    );

    f.go_to_marker(t, "fix");
    f.verify_import_fix_at_position(
        t,
        &[r#"import * as Effect from "effect/Effect";

Effect.runPromiseExit();
"#],
        Some(&preferences),
    );
    f.done(t);
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:64 TestAutoImportEffectStyleConsistency_namespaceReexport
fn test_auto_import_effect_style_consistency_namespace_reexport(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
const completion = Effect/*completion*/;
// @Filename: /mainFix.ts
const fix = Effect/*fix*/;
"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    let preferences = completion_preferences();
    let completion = "completion";

    f.verify_apply_code_action_from_completion(
        t,
        completion,
        &ApplyCodeActionFromCompletionOptions {
            name: "Effect".to_string(),
            source: "effect/Effect".to_string(),
            description: "Add import from \"effect/Effect\"".to_string(),
            new_file_content: Some(
                r#"import * as Effect from "effect/Effect";

const completion = Effect;"#
                    .to_string(),
            ),
            user_preferences: Some(preferences.clone()),
            ..Default::default()
        },
    );

    f.verify_import_fix_module_specifiers(t, "fix", &["effect/Effect"], Some(&preferences));
    f.done(t);
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:106 TestAutoImportEffectStyleConsistency_barrel
fn test_auto_import_effect_style_consistency_barrel(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "barrelImportPackages": ["EFFECT"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
succeed/*completion*/(1);
// @Filename: /mainFix.ts
succeed/*fix*/(1);
"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    let preferences = completion_preferences();
    f.go_to_marker(t, "fix");
    // Barrel imports preserve the namespace-qualified call shape and use the
    // public group barrel for nested modules.
    verify_import_fix_contents_unordered(
        t,
        &mut f,
        "fix",
        "succeed(1);\n",
        &[
            "import { Channel } from \"effect\";\n\nChannel.succeed(1);\n",
            "import { Config } from \"effect\";\n\nConfig.succeed(1);\n",
            "import { Deferred } from \"effect\";\n\nDeferred.succeed(1);\n",
            "import { DurableDeferred } from \"effect/workflow\";\n\nDurableDeferred.succeed(1);\n",
            "import { Effect } from \"effect\";\n\nEffect.succeed(1);\n",
            "import { Exit } from \"effect\";\n\nExit.succeed(1);\n",
            "import { Layer } from \"effect\";\n\nLayer.succeed(1);\n",
            "import { Prompt } from \"effect/cli\";\n\nPrompt.succeed(1);\n",
            "import { Request } from \"effect\";\n\nRequest.succeed(1);\n",
            "import { Result } from \"effect\";\n\nResult.succeed(1);\n",
            "import { SchemaGetter } from \"effect\";\n\nSchemaGetter.succeed(1);\n",
            "import { Sink } from \"effect\";\n\nSink.succeed(1);\n",
            "import { Stream } from \"effect\";\n\nStream.succeed(1);\n",
            "import { TxDeferred } from \"effect\";\n\nTxDeferred.succeed(1);\n",
        ],
        Some(&preferences),
    );
    f.done(t);
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:154 verifyImportFixContentsUnordered
// Rust: Go's `client.NextID()` + `client.SendRequestWorker` + `UnmarshalResult` is the client's `send_request`; a
// result that does not decode is Go's unmarshal error.
fn verify_import_fix_contents_unordered(
    t: &T,
    f: &mut FourslashTest,
    marker_name: &str,
    original_content: &str,
    expected: &[&str],
    preferences: Option<&UserPreferences>,
) {
    f.go_to_marker(t, marker_name);
    // Rust-only: Go's `defer reset()`: the original preferences are restored at the end.
    let reset = preferences.map(|preferences| f.configure_with_reset(t, preferences.clone()));

    let Some(marker) = f.marker_by_name(t, marker_name) else {
        t.fatal(format!("Marker '{marker_name}' not found"));
    };
    let uri = lsconv::file_name_to_document_uri(marker.file_name());
    let marker_ls_position = marker.ls_position;
    let client = f.client();

    let (_, diag_result) = client.send_request(
        lsproto::TEXT_DOCUMENT_DIAGNOSTIC_INFO,
        lsproto::DocumentDiagnosticParams {
            text_document: lsproto::TextDocumentIdentifier { uri: uri.clone() },
            ..Default::default()
        },
    );
    let Some(diag_result) = diag_result else {
        t.fatal("failed to unmarshal diagnostic response");
    };

    let mut diagnostics: Vec<lsproto::Diagnostic> = Vec::new();
    if let Some(report) = diag_result.full_document_diagnostic_report {
        diagnostics = report.items;
    }

    let (_, action_result) = client.send_request(
        lsproto::TEXT_DOCUMENT_CODE_ACTION_INFO,
        lsproto::CodeActionParams {
            text_document: lsproto::TextDocumentIdentifier { uri },
            range: lsproto::Range { start: marker_ls_position, end: marker_ls_position },
            context: lsproto::CodeActionContext { diagnostics, ..Default::default() },
            ..Default::default()
        },
    );
    let Some(action_result) = action_result else {
        t.fatal("failed to unmarshal code action response");
    };

    let mut actual: Vec<String> = Vec::new();
    let line_map = lsconv::compute_lsp_line_starts(original_content);
    if let Some(items) = &action_result.command_or_code_action_array {
        for item in items {
            let Some(code_action) = &item.code_action else {
                continue;
            };
            if code_action.kind != Some(lsproto::CodeActionKind::QuickFix) {
                continue;
            }
            let Some(changes) = code_action.edit.as_ref().and_then(|edit| edit.changes.as_ref())
            else {
                continue;
            };
            for (_, edits) in changes {
                actual.push(apply_text_edits(original_content, &line_map, edits));
            }
        }
    }

    actual.sort();
    let mut expected: Vec<&str> = expected.to_vec();
    expected.sort_unstable();
    if actual != expected {
        t.fatal(format!(
            "Unexpected import fix contents.\nExpected: {expected:?}\nActual: {actual:?}"
        ));
    }
    if let Some(reset) = reset {
        f.configure(t, reset);
    }
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:225 applyTextEdits
fn apply_text_edits(
    original: &str,
    line_map: &Arc<LSPLineMap>,
    edits: &[lsproto::TextEdit],
) -> String {
    let mut sorted_edits = edits.to_vec();
    ets_go::slices::sort_func(&mut sorted_edits, |a, b| {
        if a.range.start.line != b.range.start.line {
            return b.range.start.line.cmp(&a.range.start.line);
        }
        if a.range.start.character != b.range.start.character {
            return b.range.start.character.cmp(&a.range.start.character);
        }
        if a.range.end.line != b.range.end.line {
            return b.range.end.line.cmp(&a.range.end.line);
        }
        if a.range.end.character != b.range.end.character {
            return b.range.end.character.cmp(&a.range.end.character);
        }
        a.new_text.len().cmp(&b.new_text.len())
    });

    let mut content = original.to_string();
    let mut line_map = Arc::clone(line_map);
    for edit in &sorted_edits {
        let start = lsp_position_to_offset(&content, &line_map, edit.range.start);
        let end = lsp_position_to_offset(&content, &line_map, edit.range.end);
        content = content[..start].to_string() + &edit.new_text + &content[end..];
        line_map = lsconv::compute_lsp_line_starts(&content);
    }
    content
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:253 lspPositionToOffset
fn lsp_position_to_offset(text: &str, line_map: &LSPLineMap, mut pos: lsproto::Position) -> usize {
    let line_start = line_map.line_starts[pos.line as usize] as usize;
    let line_text = &text[line_start..];
    for (i, r) in line_text.char_indices() {
        if pos.character == 0 {
            return line_start + i;
        }
        if r == '\n' || r == '\r' {
            return line_start + i;
        }
        pos.character -= 1;
    }
    text.len()
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:268 TestAutoImportEffectStyleConsistency_topLevelNamedReexportsIgnore
fn test_auto_import_effect_style_consistency_top_level_named_reexports_ignore(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"],
        "topLevelNamedReexports": "ignore"
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
succeed/*completion*/(1);
// @Filename: /mainFix.ts
succeed/*fix*/(1);
"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    let preferences = completion_preferences();
    let completion = "completion";
    let _ = completion;

    f.go_to_marker(t, "fix");
    // With the real effect-v4 package mounted, succeed is surfaced from multiple
    // namespace modules rather than as a direct named import from "effect".
    f.verify_import_fix_at_position(
        t,
        &[
            "import * as Channel from \"effect/Channel\";\n\nChannel.succeed(1);\n",
            "import * as Config from \"effect/Config\";\n\nConfig.succeed(1);\n",
            "import * as Deferred from \"effect/Deferred\";\n\nDeferred.succeed(1);\n",
            "import * as Effect from \"effect/Effect\";\n\nEffect.succeed(1);\n",
            "import * as Exit from \"effect/Exit\";\n\nExit.succeed(1);\n",
            "import * as Layer from \"effect/Layer\";\n\nLayer.succeed(1);\n",
            "import * as Request from \"effect/Request\";\n\nRequest.succeed(1);\n",
            "import * as Result from \"effect/Result\";\n\nResult.succeed(1);\n",
            "import * as SchemaGetter from \"effect/SchemaGetter\";\n\nSchemaGetter.succeed(1);\n",
            "import * as Sink from \"effect/Sink\";\n\nSink.succeed(1);\n",
            "import * as Stream from \"effect/Stream\";\n\nStream.succeed(1);\n",
            "import * as TxDeferred from \"effect/TxDeferred\";\n\nTxDeferred.succeed(1);\n",
            "import * as Prompt from \"effect/cli/Prompt\";\n\nPrompt.succeed(1);\n",
            "import * as DurableDeferred from \"effect/workflow/DurableDeferred\";\n\nDurableDeferred.succeed(1);\n",
        ],
        Some(&preferences),
    );
    f.done(t);
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:320 TestAutoImportEffectStyleConsistency_topLevelNamedReexportsFollow
fn test_auto_import_effect_style_consistency_top_level_named_reexports_follow(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"],
        "topLevelNamedReexports": "follow"
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
runPromiseExit/*completion*/();
// @Filename: /mainFix.ts
runPromiseExit/*fix*/();
"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    let preferences = completion_preferences();

    // With topLevelNamedReexports="follow", the reexport from "effect" is suppressed,
    // leaving only the direct namespace import from "effect/Effect".
    f.go_to_marker(t, "fix");
    f.verify_import_fix_at_position(
        t,
        &[r#"import * as Effect from "effect/Effect";

Effect.runPromiseExit();
"#],
        Some(&preferences),
    );
    f.done(t);
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:358 TestAutoImportEffectStyleConsistency_testClockWithNamespace
fn test_auto_import_effect_style_consistency_test_clock_with_namespace(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainCompletion.ts
testClockWith/*completion*/(() => undefined as any);
// @Filename: /mainFix.ts
testClockWith/*fix*/(() => undefined as any);
"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    let preferences = completion_preferences();
    let completion = "completion";

    f.verify_apply_code_action_from_completion(
        t,
        completion,
        &ApplyCodeActionFromCompletionOptions {
            name: "testClockWith".to_string(),
            source: "effect/testing/TestClock".to_string(),
            description: "Add import from \"effect/testing/TestClock\"".to_string(),
            new_file_content: Some(
                r#"import * as TestClock from "effect/testing/TestClock";

TestClock.testClockWith(() => undefined as any);"#
                    .to_string(),
            ),
            user_preferences: Some(preferences.clone()),
            ..Default::default()
        },
    );

    f.go_to_marker(t, "fix");
    f.verify_import_fix_at_position(
        t,
        &[r#"import * as TestClock from "effect/testing/TestClock";

TestClock.testClockWith(() => undefined as any);
"#],
        Some(&preferences),
    );
    f.done(t);
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:404 TestAutoImportEffectStyleConsistency_testClockWithNamespaceAlongsideNamedImport
fn test_auto_import_effect_style_consistency_test_clock_with_namespace_alongside_named_import(
    t: &T,
) {
    ets_effecttest::init_hooks();
    t.parallel();
    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainFix.ts
import { adjust } from "effect/testing/TestClock"

void adjust
testClockWith/*fix*/(() => undefined as any);
"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    let preferences = completion_preferences();

    f.go_to_marker(t, "fix");
    f.verify_import_fix_at_position(
        t,
        &[r#"import * as TestClock from "effect/testing/TestClock";
import { adjust } from "effect/testing/TestClock"

void adjust
TestClock.testClockWith(() => undefined as any);
"#],
        Some(&preferences),
    );
    f.done(t);
}

// go test: internal/effecttest/autoimport_style_consistency_test.go:442 TestAutoImportEffectStyleConsistency_testClockWithUsesExistingNamespaceImport
fn test_auto_import_effect_style_consistency_test_clock_with_uses_existing_namespace_import(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let content = r#"// @Filename: /tsconfig.json
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        "namespaceImportPackages": ["effect"]
      }
    ]
  }
}
// @effect-v4
// @Filename: /mainFix.ts
import * as TestClock from "effect/testing/TestClock";

void TestClock.adjust
testClockWith/*fix*/(() => undefined as any);
"#;

    let mut f = ets_fourslash::new_fourslash(t, None /*capabilities*/, content);

    let preferences = completion_preferences();

    f.go_to_marker(t, "fix");
    f.verify_import_fix_at_position(
        t,
        &[r#"import * as TestClock from "effect/testing/TestClock";

void TestClock.adjust
TestClock.testClockWith(() => undefined as any);
"#],
        Some(&preferences),
    );
    f.done(t);
}

/// Rust-only: the ported Go test functions of `autoimport_style_consistency_test.go`, one trial each.
pub(crate) fn tests() -> Vec<Trial> {
    vec![
        run_case(
            "TestAutoImportEffectStyleConsistency_namespace".to_string(),
            test_auto_import_effect_style_consistency_namespace,
        ),
        run_case(
            "TestAutoImportEffectStyleConsistency_namespaceReexport".to_string(),
            test_auto_import_effect_style_consistency_namespace_reexport,
        ),
        run_case(
            "TestAutoImportEffectStyleConsistency_barrel".to_string(),
            test_auto_import_effect_style_consistency_barrel,
        ),
        run_case(
            "TestAutoImportEffectStyleConsistency_topLevelNamedReexportsIgnore".to_string(),
            test_auto_import_effect_style_consistency_top_level_named_reexports_ignore,
        ),
        run_case(
            "TestAutoImportEffectStyleConsistency_topLevelNamedReexportsFollow".to_string(),
            test_auto_import_effect_style_consistency_top_level_named_reexports_follow,
        ),
        run_case(
            "TestAutoImportEffectStyleConsistency_testClockWithNamespace".to_string(),
            test_auto_import_effect_style_consistency_test_clock_with_namespace,
        ),
        run_case(
            "TestAutoImportEffectStyleConsistency_testClockWithNamespaceAlongsideNamedImport"
                .to_string(),
            test_auto_import_effect_style_consistency_test_clock_with_namespace_alongside_named_import,
        ),
        run_case(
            "TestAutoImportEffectStyleConsistency_testClockWithUsesExistingNamespaceImport"
                .to_string(),
            test_auto_import_effect_style_consistency_test_clock_with_uses_existing_namespace_import,
        ),
    ]
}
