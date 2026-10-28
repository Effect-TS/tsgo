use ets_etscore::Severity;
use rustc_hash::FxHashMap;

use crate::*;

// go test: internal/rules/metadata_test.go:10 TestStrictPreset f5606515d170080b
#[test]
fn test_strict_preset() {
    let strict = metadata_preset_by_name("strict");

    for current in ALL.iter() {
        let severity = strict.get(current.name);
        if current.default_severity == Severity::Off {
            assert!(severity.is_none(), "off-by-default diagnostic {:?} is included", current.name);
            continue;
        }
        match severity {
            None => panic!("enabled-by-default diagnostic {:?} is missing", current.name),
            Some(&severity) => assert!(
                severity == Severity::Error,
                "diagnostic {:?} has severity {:?}, want error",
                current.name,
                severity.to_string()
            ),
        }
    }
}

// go test: internal/rules/metadata_test.go:31 TestRecommendedPreset a7aa9ca6aad19f36
#[test]
fn test_recommended_preset() {
    let recommended = metadata_preset_by_name("recommended");
    for current in ALL.iter() {
        let severity = recommended.get(current.name);
        if current.default_severity.is_off() {
            assert!(severity.is_none(), "off-by-default diagnostic {:?} is included", current.name);
        } else if let Some(&severity) = severity {
            assert!(
                severity == current.default_severity,
                "diagnostic {:?} has severity {:?}, want default {:?}",
                current.name,
                severity.to_string(),
                current.default_severity.to_string()
            );
        } else {
            panic!("enabled-by-default diagnostic {:?} is missing", current.name);
        }
    }
}

// go test: internal/rules/metadata_test.go:50 TestMetadataPresetCatalog 274c9c97f4520957
#[test]
fn test_metadata_preset_catalog() {
    let presets = metadata_presets();
    let names: Vec<&str> = presets.iter().map(|preset| preset.name).collect();
    let want = [
        "recommended",
        "strict",
        "correctness",
        "antipattern",
        "effect-native",
        "style",
        "maintainers",
    ];
    assert_eq!(names, want, "preset names = {names:?}, want {want:?}");

    let groups = [
        ("correctness", "correctness"),
        ("antipattern", "antipattern"),
        ("effect-native", "effectNative"),
        ("style", "style"),
        ("maintainers", "maintainers"),
    ];
    for (preset_name, group) in groups {
        let preset = metadata_preset_by_name(preset_name);
        for current in ALL.iter() {
            let severity = preset.get(current.name);
            if current.group != group {
                assert!(
                    severity.is_none(),
                    "preset {preset_name:?} includes diagnostic {:?} from group {:?}",
                    current.name,
                    current.group
                );
                continue;
            }
            if !current.replaced_by.is_empty() {
                assert!(
                    severity.is_none(),
                    "preset {preset_name:?} includes deprecated diagnostic {:?}",
                    current.name
                );
                continue;
            }
            match severity {
                None => panic!("preset {preset_name:?} is missing diagnostic {:?}", current.name),
                Some(&severity) => assert!(
                    severity == Severity::Warning,
                    "preset {preset_name:?} diagnostic {:?} has severity {:?}, want warning",
                    current.name,
                    severity.to_string()
                ),
            }
        }
    }
}

// go test: internal/rules/metadata_test.go:95 metadataPresetByName af65422e2357ae76
fn metadata_preset_by_name(name: &str) -> FxHashMap<String, Severity> {
    for preset in metadata_presets() {
        if preset.name == name {
            return preset.diagnostic_severity;
        }
    }
    panic!("preset {name:?} not found");
}
