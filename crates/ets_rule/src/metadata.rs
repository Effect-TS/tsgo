use ets_etscore::Severity;
use rustc_hash::FxHashMap;

// Rust: the `json:` tags are the field names a JSON encoder must use (`id`, `name`, `description`,
// `diagnosticSeverity`); no Rust code encodes these yet.

// go: internal/rule/metadata.go:5 MetadataGroup 890cec55a1a8aa5c
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MetadataGroup {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

// go: internal/rule/metadata.go:11 MetadataPreset 8a0c7dec7ae8ce28
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MetadataPreset {
    pub name: &'static str,
    pub description: &'static str,
    pub diagnostic_severity: FxHashMap<String, Severity>,
}
