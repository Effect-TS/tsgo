use ets_etscore::Severity;
use ets_rule::{MetadataGroup, MetadataPreset};
use rustc_hash::FxHashMap;

use crate::*;

// go: internal/rules/metadata.go:10 metadataGroups 1fed1c6001bd169d
pub(crate) static METADATA_GROUPS: [MetadataGroup; 5] = [
    MetadataGroup {
        id: "correctness",
        name: "Correctness",
        description: "Wrong, unsafe, or structurally invalid code patterns.",
    },
    MetadataGroup {
        id: "antipattern",
        name: "Anti-pattern",
        description: "Discouraged patterns that often lead to bugs or confusing behavior.",
    },
    MetadataGroup {
        id: "effectNative",
        name: "Effect-native",
        description: "Prefer Effect-native APIs and abstractions when available.",
    },
    MetadataGroup {
        id: "style",
        name: "Style",
        description: "Cleanup, consistency, and idiomatic Effect code.",
    },
    MetadataGroup {
        id: "maintainers",
        name: "Maintainers",
        description: "Guard the public API surface and release hygiene of a package.",
    },
];

// go: internal/rules/metadata.go:18 MetadataGroups c054d5dd3acb01e2
pub fn metadata_groups() -> Vec<MetadataGroup> {
    METADATA_GROUPS.to_vec()
}

// go: internal/rules/metadata.go:22 MetadataPresets 16f93b733f29d0c5
pub fn metadata_presets() -> Vec<MetadataPreset> {
    vec![
        MetadataPreset {
            name: "recommended",
            description: "Enable diagnostics at their default severities.",
            diagnostic_severity: build_default_preset(),
        },
        MetadataPreset {
            name: "strict",
            description: "Treat every diagnostic enabled by default as an error.",
            diagnostic_severity: build_strict_preset(),
        },
        MetadataPreset {
            name: "correctness",
            description: "Enable all correctness diagnostics at warning level.",
            diagnostic_severity: build_group_preset("correctness", Severity::Warning),
        },
        MetadataPreset {
            name: "antipattern",
            description: "Enable all anti-pattern diagnostics at warning level.",
            diagnostic_severity: build_group_preset("antipattern", Severity::Warning),
        },
        MetadataPreset {
            name: "effect-native",
            description: "Enable all Effect-native diagnostics at warning level.",
            diagnostic_severity: build_group_preset("effectNative", Severity::Warning),
        },
        MetadataPreset {
            name: "style",
            description: "Enable all style diagnostics at warning level.",
            diagnostic_severity: build_group_preset("style", Severity::Warning),
        },
        MetadataPreset {
            name: "maintainers",
            description: "Enable all maintainer diagnostics at warning level.",
            diagnostic_severity: build_group_preset("maintainers", Severity::Warning),
        },
    ]
}

// go: internal/rules/metadata.go:62 buildGroupPreset 18223ae28c98d174
pub(crate) fn build_group_preset(group: &str, severity: Severity) -> FxHashMap<String, Severity> {
    let mut preset = FxHashMap::default();
    for current in ALL.iter() {
        // Deprecated rules duplicate their replacements and would make the
        // written configuration report deprecatedRuleName.
        if current.group == group && current.replaced_by.is_empty() {
            preset.insert(current.name.to_string(), severity);
        }
    }
    preset
}

// go: internal/rules/metadata.go:74 buildStrictPreset 44def83207a81ae1
pub(crate) fn build_strict_preset() -> FxHashMap<String, Severity> {
    let mut preset = build_default_preset();
    for severity in preset.values_mut() {
        *severity = Severity::Error;
    }
    preset
}

// go: internal/rules/metadata.go:82 buildDefaultPreset 2ee9dffa4274ca30
pub(crate) fn build_default_preset() -> FxHashMap<String, Severity> {
    let mut preset = FxHashMap::default();
    for current in ALL.iter() {
        if !current.default_severity.is_off() {
            preset.insert(current.name.to_string(), current.default_severity);
        }
    }
    preset
}
