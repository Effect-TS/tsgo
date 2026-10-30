use std::sync::LazyLock;

use rustc_hash::FxHashMap;

/// Rust-only: `toKebabCase` of `_tools/repoctl/src/codegen.ts`, which names the tsgolint Effect rules.
pub(crate) fn to_kebab_case(value: &str) -> String {
    let characters: Vec<char> = value.chars().collect();
    let mut result = String::new();
    for (index, &current) in characters.iter().enumerate() {
        if current.is_ascii_uppercase() {
            let previous_is_lower = index > 0 && characters[index - 1].is_ascii_lowercase();
            let next_is_lower = characters.get(index + 1).is_some_and(char::is_ascii_lowercase);
            if index > 0 && (previous_is_lower || next_is_lower) {
                result.push('-');
            }
            result.push(current.to_ascii_lowercase());
        } else {
            result.push(current);
        }
    }
    result
}

/// Rust-only: the `newEffectRule(qualifiedName, effectName)` registrations of `renderTsgolintEffectRules`.
#[cfg_attr(
    not(any(feature = "cli", test)),
    expect(dead_code, reason = "only the linter looks rules up")
)]
pub(crate) static EFFECT_RULE_NAMES: LazyLock<FxHashMap<String, &'static str>> =
    LazyLock::new(|| {
        ets_rules::ALL
            .iter()
            .map(|rule| (format!("effecttsgo/{}", to_kebab_case(rule.name)), rule.name))
            .collect()
    });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_case_matches_codegen() {
        for (input, expected) in [
            ("floatingEffect", "floating-effect"),
            ("missingEffectContext", "missing-effect-context"),
            ("globalJSON", "global-json"),
            ("globalJsonInEffect", "global-json-in-effect"),
            ("anyUnknownInErrorContext", "any-unknown-in-error-context"),
        ] {
            assert_eq!(to_kebab_case(input), expected, "{input}");
        }
    }

    #[test]
    fn every_effect_rule_has_a_distinct_name() {
        assert_eq!(EFFECT_RULE_NAMES.len(), ets_rules::ALL.len());
        assert_eq!(EFFECT_RULE_NAMES.get("effecttsgo/floating-effect"), Some(&"floatingEffect"));
    }
}
