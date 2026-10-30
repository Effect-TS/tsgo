// Port of etscore/options_schema_test.go (package etscore_test).
//
// Rust: Go's `map[string]any` documents are the Rust-only `Any` below (the cases `encoding/json` decodes into
// `any`, plus the struct the generator marshals), with a Go map's reference semantics (`Rc<RefCell<..>>`: the
// generator writes through maps it reached by type assertion). `reflect.TypeOf(etscore.X{})` is the Rust-only
// `ets_etscore::X::TYPE` (`ets_go::reflect`), which lists each Go field with its verbatim struct tag.
// `json.Marshal` / `json.MarshalIndent` are the Rust-only `marshal_indent`, Go's encoding/json encoder for these
// values (sorted map keys, HTML-safe string escaping, Go's float format); `json.Unmarshal` decodes with tsrs's
// JSON parser and converts its value.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ets_etscore::{
    DEFAULT_KEY_PATTERNS, EFFECT_PLUGIN_NAME, EffectPluginOptions, KeyPattern, Override,
    OverrideOptions, Severity,
};
use ets_go::reflect::{Kind, StructField, StructType};
use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::core::json::{self, Value};

/// Rust-only: a Go `map[string]any`. Go maps are references; `BTreeMap` keeps the keys in the order
/// `encoding/json` writes them (sorted by bytes).
type Map = Rc<RefCell<BTreeMap<String, Any>>>;

/// Rust-only: the values Go's `any` holds in this file.
#[derive(Clone, Debug, PartialEq)]
enum Any {
    Nil,
    Bool(bool),
    Float64(f64),
    String(String),
    // `[]any` and `[]string`.
    Slice(Vec<Any>),
    Map(Map),
    // A struct value, encoded by `encoding/json` with its fields' json names in declaration order.
    Struct(Vec<(String, Any)>),
}

impl Any {
    /// Rust-only: Go's `value.(map[string]any)`.
    fn as_map(&self) -> Option<Map> {
        match self {
            Any::Map(map) => Some(Rc::clone(map)),
            _ => None,
        }
    }

    /// Rust-only: Go's `[]string{...}` literal.
    fn strings(values: &[&str]) -> Any {
        Any::Slice(values.iter().map(|value| Any::String(value.to_string())).collect())
    }
}

/// Rust-only: Go's `map[string]any{...}` literal.
fn map_of(entries: Vec<(&str, Any)>) -> Map {
    let mut map = BTreeMap::new();
    for (key, value) in entries {
        map.insert(key.to_string(), value);
    }
    Rc::new(RefCell::new(map))
}

/// Rust-only: `map_of` as an `any`.
fn object(entries: Vec<(&str, Any)>) -> Any {
    Any::Map(map_of(entries))
}

/// Rust-only: Go's `m[key].(map[string]any)`.
fn get_map(map: &Map, key: &str) -> Option<Map> {
    map.borrow().get(key).and_then(Any::as_map)
}

/// Rust-only: Go's `m[key] = value`.
fn set(map: &Map, key: &str, value: Any) {
    map.borrow_mut().insert(key.to_string(), value);
}

/// Rust-only: Go's `json.Unmarshal(data, &decoded)` into an `any` (objects become `map[string]any`, numbers
/// `float64`).
fn unmarshal(data: &str) -> Result<Any, String> {
    fn convert(value: Value) -> Any {
        match value {
            Value::Null => Any::Nil,
            Value::Bool(b) => Any::Bool(b),
            Value::Number(n) => Any::Float64(n),
            Value::Integer(n) => Any::Float64(n as f64),
            Value::String(s) => Any::String(s),
            Value::Array(items) => Any::Slice(items.into_iter().map(convert).collect()),
            Value::Object(members) => {
                let mut map = BTreeMap::new();
                for (key, value) in members {
                    map.insert(key, convert(value));
                }
                Any::Map(Rc::new(RefCell::new(map)))
            }
        }
    }
    json::unmarshal(data).map(convert)
}

/// Rust-only: Go's `json.MarshalIndent(v, prefix, indent)` (and `json.Marshal(v)` with both empty) with
/// `encoding/json`'s output: map keys sorted, `{}` / `[]` for empty maps and slices, `"key": value` when indenting.
fn marshal_indent(v: &Any, prefix: &str, indent: &str) -> String {
    fn newline(out: &mut String, prefix: &str, indent: &str, depth: usize) {
        if prefix.is_empty() && indent.is_empty() {
            return;
        }
        out.push('\n');
        out.push_str(prefix);
        for _ in 0..depth {
            out.push_str(indent);
        }
    }
    fn write_members<'a>(
        out: &mut String,
        members: impl ExactSizeIterator<Item = (&'a String, &'a Any)>,
        prefix: &str,
        indent: &str,
        depth: usize,
    ) {
        out.push('{');
        let empty = members.len() == 0;
        for (i, (key, value)) in members.enumerate() {
            if i > 0 {
                out.push(',');
            }
            newline(out, prefix, indent, depth + 1);
            write_string(out, key);
            out.push(':');
            if !indent.is_empty() || !prefix.is_empty() {
                out.push(' ');
            }
            write(out, value, prefix, indent, depth + 1);
        }
        if !empty {
            newline(out, prefix, indent, depth);
        }
        out.push('}');
    }
    fn write(out: &mut String, v: &Any, prefix: &str, indent: &str, depth: usize) {
        match v {
            Any::Nil => out.push_str("null"),
            Any::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Any::Float64(f) => write_float64(out, *f),
            Any::String(s) => write_string(out, s),
            Any::Slice(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    newline(out, prefix, indent, depth + 1);
                    write(out, item, prefix, indent, depth + 1);
                }
                if !items.is_empty() {
                    newline(out, prefix, indent, depth);
                }
                out.push(']');
            }
            Any::Map(map) => write_members(out, map.borrow().iter(), prefix, indent, depth),
            Any::Struct(fields) => {
                write_members(out, fields.iter().map(|(k, v)| (k, v)), prefix, indent, depth);
            }
        }
    }
    // encoding/json's `floatEncoder` for float64.
    fn write_float64(out: &mut String, f: f64) {
        let abs = f.abs();
        let mut fmt = b'f';
        if abs != 0.0 && (abs < 1e-6 || abs >= 1e21) {
            fmt = b'e';
        }
        let mut b = ets_go::strconv::format_float(f, fmt, -1, 64).into_bytes();
        if fmt == b'e' {
            // clean up e-09 to e-9
            let n = b.len();
            if n >= 4 && b[n - 4] == b'e' && b[n - 3] == b'-' && b[n - 2] == b'0' {
                b[n - 2] = b[n - 1];
                b.truncate(n - 1);
            }
        }
        out.push_str(std::str::from_utf8(&b).expect("ASCII"));
    }
    // encoding/json's `appendString` with HTML escaping on (`json.Marshal`'s default). A Rust string is valid
    // UTF-8, so Go's U+FFFD replacement of invalid bytes never applies.
    fn write_string(out: &mut String, s: &str) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
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
                '\u{2028}' => out.push_str("\\u2028"),
                '\u{2029}' => out.push_str("\\u2029"),
                c if (c as u32) < 0x20 || c == '<' || c == '>' || c == '&' => {
                    let b = c as u32 as u8;
                    out.push_str("\\u00");
                    out.push(HEX[(b >> 4) as usize] as char);
                    out.push(HEX[(b & 0xF) as usize] as char);
                }
                c => out.push(c),
            }
        }
        out.push('"');
    }
    let mut out = String::new();
    write(&mut out, v, prefix, indent, 0);
    out
}

/// Rust-only: Go's `encoding/json` encoding of a `[]etscore.KeyPattern` value: each struct by its fields' json
/// names (`etscore.KeyPattern`'s tags), a nil slice as `null`.
fn key_patterns_json(patterns: &[KeyPattern]) -> Any {
    Any::Slice(
        patterns
            .iter()
            .map(|pattern| {
                let mut fields = Vec::new();
                for field in KeyPattern::TYPE.fields {
                    let value = match field.name {
                        "Target" => Any::String(pattern.target.clone()),
                        "Pattern" => Any::String(pattern.pattern.clone()),
                        "SkipLeadingPath" => match &pattern.skip_leading_path {
                            Some(paths) => {
                                Any::Slice(paths.iter().cloned().map(Any::String).collect())
                            }
                            None => Any::Nil,
                        },
                        name => panic!("unexpected KeyPattern field {name}"),
                    };
                    fields.push((json_field_name(field), value));
                }
                Any::Struct(fields)
            })
            .collect(),
    )
}

// go test: etscore/options_schema_test.go:19 diagnosticRuleSchema 85bafb7a89379d36
struct DiagnosticRuleSchema {
    name: String,
    description: String,
    default_severity: Severity,
}

// go test: etscore/options_schema_test.go:25 TestGenerateTSConfigSchemaMatchesReference 140cd5e757c023cb
#[test]
fn test_generate_tsconfig_schema_matches_reference() {
    ets_go::testing::run(|t| {
        let actual = match generate_tsconfig_schema() {
            Ok(actual) => actual,
            Err(err) => t.fatal(sprintf!("generateTSConfigSchema() error = %v", err)),
        };
        assert_matches_reference(t, "schema.json", &actual, "UPDATE_TSCONFIG_SCHEMA");
    });
}

// go test: etscore/options_schema_test.go:33 assertMatchesReference 8a2aaa513ad378da
// Rust: the Rust runner writes its local copy to `local-rs/` (Go's is `local/`), and never updates the
// reference: Go's `updateEnv` branch is left out (the Go test owns the reference).
fn assert_matches_reference(t: &T, name: &str, actual: &[u8], _update_env: &str) {
    t.helper();
    let root = repo_root();
    let local_path = root.join("testdata").join("baselines").join("local-rs").join(name);
    let reference_path = root.join(name);

    write_if_changed(t, &local_path, actual);

    let expected = match std::fs::read(&reference_path) {
        Ok(expected) => expected,
        Err(err) => t.fatal(sprintf!(
            "failed to read reference schema %q: %v",
            reference_path.display().to_string(),
            err.to_string()
        )),
    };
    if actual == expected {
        return;
    }

    let diff = diff_text(
        &reference_path.display().to_string(),
        &local_path.display().to_string(),
        &String::from_utf8_lossy(&expected),
        &String::from_utf8_lossy(actual),
    );
    let diff_lines: Vec<String> = diff.split('\n').map(|line| format!("  {line}")).collect();
    t.fatal(sprintf!("%s is out of date:\n%s", name, diff_lines.join("\n")));
}

/// Rust-only: Go's `baseline.DiffText(oldName, newName, expected, actual)`, as `ets_effecttest::diff_text`
/// ports it: a unified diff of the reference and the actual file.
fn diff_text(old_name: &str, new_name: &str, expected: &str, actual: &str) -> String {
    similar::TextDiff::from_lines(expected, actual)
        .unified_diff()
        .header(old_name, new_name)
        .to_string()
}

// go test: etscore/options_schema_test.go:61 repoRoot edff299b3ea0d8e7
// Rust: Go's `runtime.Caller(0)` file is etscore/options_schema_test.go; the repository root is two levels up
// from this crate's manifest directory.
fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    match manifest_dir.parent().and_then(Path::parent) {
        Some(root) => root.to_path_buf(),
        None => panic!("failed to resolve caller path"),
    }
}

// go test: etscore/options_schema_test.go:69 schemaRules 975bc927c0316359
fn schema_rules() -> Vec<DiagnosticRuleSchema> {
    let mut result = Vec::with_capacity(ets_rules::ALL.len());
    for rule in ets_rules::ALL.iter() {
        result.push(DiagnosticRuleSchema {
            name: rule.name.to_string(),
            description: rule.description.to_string(),
            default_severity: rule.default_severity,
        });
    }
    result
}

// go test: etscore/options_schema_test.go:81 generateTSConfigSchema 2c206f4fa208afb9
fn generate_tsconfig_schema() -> Result<Vec<u8>, String> {
    let root = repo_root();
    let base_schema_path = root.join("_tools").join("tsconfig-base-schema.json");
    let base_schema_content =
        std::fs::read_to_string(&base_schema_path).map_err(|err| err.to_string())?;

    let Some(document) = unmarshal(&base_schema_content)?.as_map() else {
        return Err(
            "json: cannot unmarshal into Go value of type map[string]interface {}".to_string()
        );
    };

    let Some(definitions) = get_map(&document, "definitions") else {
        return Err("definitions not found in base schema".to_string());
    };
    let Some(compiler_options_definition) = get_map(&definitions, "compilerOptionsDefinition")
    else {
        return Err("compilerOptionsDefinition not found in base schema".to_string());
    };
    let compiler_options =
        nested_object(&compiler_options_definition, &["properties", "compilerOptions"])?;
    let compiler_options_variant =
        schema_variant_with_object_field(&compiler_options, "properties")
            .map_err(|err| format!("compilerOptions: {err}"))?;
    let compiler_options_properties =
        get_map(&compiler_options_variant, "properties").expect("map[string]any");
    let Some(plugins) = get_map(&compiler_options_properties, "plugins") else {
        return Err("compilerOptions.plugins not found in base schema".to_string());
    };
    let plugins_variant = schema_variant_with_object_field(&plugins, "items")
        .map_err(|err| format!("compilerOptions.plugins: {err}"))?;
    let plugin_items = get_map(&plugins_variant, "items").expect("map[string]any");

    set(
        &definitions,
        "effectLanguageServicePluginOptionsDefinition",
        Any::Map(effect_language_service_plugin_options_schema()),
    );
    set(
        &definitions,
        "effectLanguageServicePluginDiagnosticSeverityDefinition",
        Any::Map(diagnostic_severity_schema()),
    );
    set(&definitions, "effectLanguageServicePluginSeverityDefinition", Any::Map(severity_schema()));
    set(
        &definitions,
        "effectLanguageServicePluginKeyPatternDefinition",
        Any::Map(reflected_schema_for_struct(&KeyPattern::TYPE)),
    );
    set(
        &definitions,
        "effectLanguageServicePluginOverrideDefinition",
        Any::Map(effect_language_service_plugin_override_schema()),
    );
    set(
        &definitions,
        "effectLanguageServicePluginOverrideOptionsDefinition",
        Any::Map(effect_language_service_plugin_override_options_schema()),
    );

    let items = object(vec![(
        "anyOf",
        Any::Slice(vec![
            Any::Map(effect_language_service_plugin_schema()),
            Any::Map(other_plugin_schema(&plugin_items)),
        ]),
    )]);
    set(&plugins_variant, "items", items);

    let mut output = marshal_indent(&Any::Map(document), "", "  ").into_bytes();
    output.push(b'\n');
    Ok(output)
}

// go test: etscore/options_schema_test.go:146 effectLanguageServicePluginSchema f683771855d4adc5
fn effect_language_service_plugin_schema() -> Map {
    let options_schema = effect_language_service_plugin_options_schema();
    let properties =
        clone_properties(&get_map(&options_schema, "properties").expect("map[string]any"));
    let schema = map_of(vec![
        ("type", Any::String("object".to_string())),
        ("title", Any::String("@effect/language-service plugin".to_string())),
        ("additionalProperties", Any::Bool(false)),
        ("properties", Any::Map(Rc::clone(&properties))),
        ("required", Any::strings(&["name"])),
    ]);
    set(
        &properties,
        "name",
        object(vec![
            ("type", Any::String("string".to_string())),
            ("const", Any::String(EFFECT_PLUGIN_NAME.to_string())),
        ]),
    );
    schema
}

// go test: etscore/options_schema_test.go:163 effectLanguageServicePluginOptionsSchema d966a202db54672e
fn effect_language_service_plugin_options_schema() -> Map {
    let schema = reflected_schema_for_struct(&EffectPluginOptions::TYPE);
    let properties = get_map(&schema, "properties").expect("map[string]any");
    set(
        &properties,
        "diagnosticSeverity",
        object(vec![
            (
                "$ref",
                Any::String(
                    "#/definitions/effectLanguageServicePluginDiagnosticSeverityDefinition"
                        .to_string(),
                ),
            ),
            (
                "description",
                Any::String(struct_field_tag(
                    &EffectPluginOptions::TYPE,
                    "DiagnosticSeverity",
                    "schema_description",
                )),
            ),
            ("default", object(vec![])),
        ]),
    );
    set(
        &properties,
        "keyPatterns",
        object(vec![
            ("type", Any::String("array".to_string())),
            (
                "description",
                Any::String(struct_field_tag(
                    &EffectPluginOptions::TYPE,
                    "KeyPatterns",
                    "schema_description",
                )),
            ),
            ("default", key_patterns_json(&DEFAULT_KEY_PATTERNS)),
            (
                "items",
                object(vec![(
                    "$ref",
                    Any::String(
                        "#/definitions/effectLanguageServicePluginKeyPatternDefinition".to_string(),
                    ),
                )]),
            ),
        ]),
    );
    set(
        &properties,
        "overrides",
        object(vec![
            ("type", Any::String("array".to_string())),
            (
                "description",
                Any::String(struct_field_tag(
                    &EffectPluginOptions::TYPE,
                    "Overrides",
                    "schema_description",
                )),
            ),
            (
                "items",
                object(vec![(
                    "$ref",
                    Any::String(
                        "#/definitions/effectLanguageServicePluginOverrideDefinition".to_string(),
                    ),
                )]),
            ),
        ]),
    );
    schema
}

// go test: etscore/options_schema_test.go:189 effectLanguageServicePluginOverrideSchema 720a8d76c0744470
fn effect_language_service_plugin_override_schema() -> Map {
    let schema = reflected_schema_for_struct(&Override::TYPE);
    let properties = get_map(&schema, "properties").expect("map[string]any");
    set(
        &properties,
        "options",
        object(vec![
            (
                "$ref",
                Any::String(
                    "#/definitions/effectLanguageServicePluginOverrideOptionsDefinition"
                        .to_string(),
                ),
            ),
            (
                "description",
                Any::String(struct_field_tag(&Override::TYPE, "Options", "schema_description")),
            ),
        ]),
    );
    schema
}

// go test: etscore/options_schema_test.go:199 effectLanguageServicePluginOverrideOptionsSchema 1deab256a280fa0b
fn effect_language_service_plugin_override_options_schema() -> Map {
    let schema = reflected_schema_for_struct(&OverrideOptions::TYPE);
    let properties = get_map(&schema, "properties").expect("map[string]any");
    set(
        &properties,
        "diagnosticSeverity",
        object(vec![
            (
                "$ref",
                Any::String(
                    "#/definitions/effectLanguageServicePluginDiagnosticSeverityDefinition"
                        .to_string(),
                ),
            ),
            (
                "description",
                Any::String(struct_field_tag(
                    &OverrideOptions::TYPE,
                    "DiagnosticSeverity",
                    "schema_description",
                )),
            ),
        ]),
    );
    set(
        &properties,
        "keyPatterns",
        object(vec![
            ("type", Any::String("array".to_string())),
            (
                "description",
                Any::String(struct_field_tag(
                    &OverrideOptions::TYPE,
                    "KeyPatterns",
                    "schema_description",
                )),
            ),
            (
                "items",
                object(vec![(
                    "$ref",
                    Any::String(
                        "#/definitions/effectLanguageServicePluginKeyPatternDefinition".to_string(),
                    ),
                )]),
            ),
        ]),
    );
    schema
}

// go test: etscore/options_schema_test.go:216 diagnosticSeveritySchema c4b1ef40abf5d101
fn diagnostic_severity_schema() -> Map {
    let properties = map_of(vec![]);
    for rule in schema_rules() {
        set(
            &properties,
            &rule.name,
            object(vec![
                (
                    "$ref",
                    Any::String(
                        "#/definitions/effectLanguageServicePluginSeverityDefinition".to_string(),
                    ),
                ),
                ("description", Any::String(rule.description)),
                ("default", Any::String(rule.default_severity.string().to_string())),
            ]),
        );
    }
    map_of(vec![
        ("type", Any::String("object".to_string())),
        (
            "description",
            Any::String(
                "Allows overriding the default severity for each Effect diagnostic across the project."
                    .to_string(),
            ),
        ),
        (
            "additionalProperties",
            object(vec![
                ("type", Any::String("string".to_string())),
                (
                    "enum",
                    Any::strings(&[
                        Severity::Off.string(),
                        Severity::Error.string(),
                        Severity::Warning.string(),
                        Severity::Message.string(),
                        Severity::Suggestion.string(),
                    ]),
                ),
            ]),
        ),
        ("properties", Any::Map(properties)),
    ])
}

// go test: etscore/options_schema_test.go:242 severitySchema fa38718a4a13bb14
fn severity_schema() -> Map {
    map_of(vec![
        ("type", Any::String("string".to_string())),
        (
            "enum",
            Any::strings(&[
                Severity::Off.string(),
                Severity::Error.string(),
                Severity::Warning.string(),
                Severity::Message.string(),
                Severity::Suggestion.string(),
            ]),
        ),
    ])
}

// go test: etscore/options_schema_test.go:255 otherPluginSchema ff17bae3ed7eb168
fn other_plugin_schema(existing: &Map) -> Map {
    let schema = clone_json(existing);
    let properties = get_map(&schema, "properties").unwrap_or_else(|| {
        let properties = map_of(vec![]);
        set(&schema, "properties", Any::Map(Rc::clone(&properties)));
        properties
    });

    let mut name_schema = map_of(vec![
        ("type", Any::String("string".to_string())),
        ("description", Any::String("Plugin name.".to_string())),
    ]);
    if let Some(existing_name_schema) = get_map(&properties, "name") {
        name_schema = clone_json(&existing_name_schema);
    }
    set(&name_schema, "not", object(vec![("enum", Any::strings(&[EFFECT_PLUGIN_NAME]))]));
    set(&properties, "name", Any::Map(name_schema));

    schema
}

// go test: etscore/options_schema_test.go:278 reflectedSchemaForStruct 85d4ae2f2e8f3781
fn reflected_schema_for_struct(typ: &StructType) -> Map {
    let properties = map_of(vec![]);
    for i in 0..typ.num_field() {
        let field = typ.field(i);
        let name = json_field_name(field);
        if name.is_empty() {
            continue;
        }
        set(&properties, &name, Any::Map(reflected_schema_for_field(field)));
    }

    map_of(vec![
        ("type", Any::String("object".to_string())),
        ("additionalProperties", Any::Bool(false)),
        ("properties", Any::Map(properties)),
    ])
}

// go test: etscore/options_schema_test.go:296 cloneProperties 1f4eb1c503dde789
fn clone_properties(properties: &Map) -> Map {
    let mut cloned = BTreeMap::new();
    for (key, value) in properties.borrow().iter() {
        cloned.insert(key.clone(), value.clone());
    }
    Rc::new(RefCell::new(cloned))
}

// go test: etscore/options_schema_test.go:304 cloneJSON 29c69e616cdf56e7
// Rust: Go's generic `cloneJSON[T]` is only instantiated with `map[string]any`.
fn clone_json(value: &Map) -> Map {
    let data = marshal_indent(&Any::Map(Rc::clone(value)), "", "");
    match unmarshal(&data) {
        Ok(cloned) => cloned.as_map().expect("map[string]any"),
        Err(err) => panic!("{err}"),
    }
}

// go test: etscore/options_schema_test.go:316 reflectedSchemaForField 2a6dfa82d5aa03e4
fn reflected_schema_for_field(field: &StructField) -> Map {
    let schema = map_of(vec![]);
    let mut field_type = field.typ;
    if field_type.kind() == Kind::Pointer {
        field_type = field_type.elem();
    }

    match field_type.kind() {
        Kind::Bool => set(&schema, "type", Any::String("boolean".to_string())),
        Kind::Int => set(&schema, "type", Any::String("integer".to_string())),
        Kind::String => set(&schema, "type", Any::String("string".to_string())),
        Kind::Slice => {
            set(&schema, "type", Any::String("array".to_string()));
            let item_type = field.tag.get("schema_items_type");
            let item_schema = map_of(vec![]);
            if !item_type.is_empty() {
                set(&item_schema, "type", Any::String(item_type));
            }
            if let Some(item_enum) = decode_json_tag(field, "schema_items_enum") {
                set(&item_schema, "enum", item_enum);
            }
            if !item_schema.borrow().is_empty() {
                set(&schema, "items", Any::Map(item_schema));
            }
        }
        Kind::Map => {
            set(&schema, "type", Any::String("object".to_string()));
            let prop_type = field.tag.get("schema_additional_properties_type");
            if !prop_type.is_empty() {
                set(
                    &schema,
                    "additionalProperties",
                    object(vec![("type", Any::String(prop_type))]),
                );
            }
        }
        Kind::Struct | Kind::Pointer => {}
    }

    let description = field.tag.get("schema_description");
    if !description.is_empty() {
        set(&schema, "description", Any::String(description));
    }
    if let Some(default_value) = decode_json_tag(field, "schema_default") {
        set(&schema, "default", default_value);
    }
    if let Some(enum_) = decode_json_tag(field, "schema_enum") {
        set(&schema, "enum", enum_);
    }
    if let Some(minimum) = decode_json_tag(field, "schema_minimum") {
        set(&schema, "minimum", minimum);
    }
    if let Some(unique_items) = decode_json_tag(field, "schema_unique_items") {
        set(&schema, "uniqueItems", unique_items);
    }

    schema
}

// go test: etscore/options_schema_test.go:369 decodeJSONTag 6c7214615e166ea4
// Rust: Go's nil `any` (an absent tag, or a tag that decodes to `null`) is `None`.
fn decode_json_tag(field: &StructField, key: &str) -> Option<Any> {
    let value = field.tag.get(key);
    if value.is_empty() {
        return None;
    }
    match unmarshal(&value) {
        Ok(Any::Nil) => None,
        Ok(decoded) => Some(decoded),
        Err(err) => panic!("{err}"),
    }
}

// go test: etscore/options_schema_test.go:381 jsonFieldName 632c44eff9e2e6a9
fn json_field_name(field: &StructField) -> String {
    let tag = field.tag.get("json");
    if tag.is_empty() {
        return String::new();
    }
    let name = tag.split(',').next().unwrap_or_default();
    if name == "-" {
        return String::new();
    }
    name.to_string()
}

// go test: etscore/options_schema_test.go:393 structFieldTag c394c05e48e11e7e
fn struct_field_tag(typ: &StructType, field_name: &str, tag_name: &str) -> String {
    let Some(field) = typ.field_by_name(field_name) else {
        panic!("missing field: {field_name}");
    };
    field.tag.get(tag_name)
}

// go test: etscore/options_schema_test.go:401 nestedObject b099684ed3fa731f
fn nested_object(root: &Map, keys: &[&str]) -> Result<Map, String> {
    let mut current = Rc::clone(root);
    for key in keys {
        let Some(next) = get_map(&current, key) else {
            return Err(sprintf!("schema path %q not found", keys.join(".")));
        };
        current = next;
    }
    Ok(current)
}

// go test: etscore/options_schema_test.go:413 schemaVariantWithObjectField 99c7ecc87d5b4cac
fn schema_variant_with_object_field(root: &Map, field: &str) -> Result<Map, String> {
    if get_map(root, field).is_some() {
        return Ok(Rc::clone(root));
    }
    if let Some(Any::Slice(variants)) = root.borrow().get("anyOf") {
        for value in variants {
            let Some(variant) = value.as_map() else {
                continue;
            };
            if get_map(&variant, field).is_some() {
                return Ok(variant);
            }
        }
    }
    Err(sprintf!("object field %q not found", field))
}

// go test: etscore/options_schema_test.go:431 writeIfChanged 1a8b568e399df379
fn write_if_changed(t: &T, path: &Path, content: &[u8]) {
    t.helper();

    let existing = std::fs::read(path);
    if let Ok(existing) = &existing
        && existing == content
    {
        return;
    }
    if let Err(err) = &existing
        && err.kind() != std::io::ErrorKind::NotFound
    {
        t.fatal(sprintf!("failed to read %q: %v", path.display().to_string(), err.to_string()));
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    if let Err(err) = std::fs::create_dir_all(dir) {
        t.fatal(sprintf!("failed to create %q: %v", dir.display().to_string(), err.to_string()));
    }
    if let Err(err) = std::fs::write(path, content) {
        t.fatal(sprintf!("failed to write %q: %v", path.display().to_string(), err.to_string()));
    }
}
