// Package directives implements parsing for @effect-diagnostics directive comments.

use std::sync::LazyLock;

use ets_etscore::{Severity, parse_severity};
use ets_shim::core::stringutil::go_strings_to_lower;
use ets_shim::diagnostics;
use regex::Regex;
use rustc_hash::FxHashMap;

// go: internal/directives/parser.go:16 ToCategory dc777d979d15644a
// ToCategory converts a Severity to the corresponding diagnostics.Category for diagnostic reporting.
// This function stays here because etscore cannot import diagnostics (leaf package).
// etscore.SeverityOff and etscore.SeveritySkipFile return CategoryWarning as a fallback (callers should
// check for these and skip reporting entirely).
pub fn to_category(s: Severity) -> diagnostics::Category {
    match s {
        Severity::Error => diagnostics::Category::Error,
        Severity::Warning => diagnostics::Category::Warning,
        Severity::Suggestion => diagnostics::Category::Suggestion,
        Severity::Message => diagnostics::Category::Message,
        _ => {
            // Off and SkipFile shouldn't reach diagnostic creation,
            // but return Warning as a safe default
            diagnostics::Category::Warning
        }
    }
}

// go: internal/directives/parser.go:34 RuleSeverity b44b6d0abcc72eb7
// RuleSeverity represents a rule:severity pair from a directive.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RuleSeverity {
    pub rule: String,
    pub severity: Severity,
}

// go: internal/directives/parser.go:40 Directive 117e6365fb4fef1d
// Directive represents a parsed @effect-diagnostics directive.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Directive {
    // Line is the 0-indexed line number where the directive appears
    pub line: i64,
    // Pos is the start position of the directive in the source text
    pub pos: i64,
    // End is the end position of the directive in the source text
    pub end: i64,
    // IsNextLine is true if this is a -next-line directive (affects line+1)
    pub is_next_line: bool,
    // Rules is the list of rule:severity pairs
    pub rules: Vec<RuleSeverity>,
}

impl Directive {
    // go: internal/directives/parser.go:56 Directive.AffectedLine 844505badfa59e4f
    // AffectedLine returns the line number that this directive affects.
    // For regular directives, it affects the same line.
    // For -next-line directives, it affects the following line.
    pub fn affected_line(&self) -> i64 {
        if self.is_next_line {
            return self.line + 1;
        }
        self.line
    }
}

// Rust: the patterns are Go's RE2 syntax spelled for the `regex` crate with Go's ASCII classes: RE2's `\s` is
// `[\t\n\f\r ]` (Rust's ASCII `\s` adds `\v`) and `\w` is `[0-9A-Za-z_]` (Rust's `\w` is Unicode). Both engines
// pick the leftmost-first match.

// go: internal/directives/parser.go:69 directivePattern a1802175f7797b3e
// directivePattern matches @effect-diagnostics(-next-line)? followed by rule:level pairs
// Examples:
//   - @effect-diagnostics floatingEffect:off
//   - @effect-diagnostics-next-line floatingEffect:off
//   - @effect-diagnostics floatingEffect:off pocRule:warning
//   - @effect-diagnostics *:skip-file
pub(crate) static DIRECTIVE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"@effect-diagnostics(-next-line)?[\t\n\f\r ]+([0-9A-Za-z_:\-*]+(?:[\t\n\f\r ]+[0-9A-Za-z_:\-*]+)*)",
    )
    .unwrap()
});

// go: internal/directives/parser.go:72 ruleSeverityPattern eaf620c94165a2a1
// ruleSeverityPattern matches a single rule:severity pair
pub(crate) static RULE_SEVERITY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([0-9A-Za-z_]+|\*):([0-9A-Za-z_]+(?:-[0-9A-Za-z_]+)?)").unwrap());

// go: internal/directives/parser.go:75 CollectEffectDirectives e7ed1c5becf6e74d
// CollectEffectDirectives scans the source text and returns all Effect directives found.
pub fn collect_effect_directives(source_text: &str) -> Vec<Directive> {
    let lines = source_text.split('\n');
    let mut directives = Vec::new();
    let mut line_start_pos: i64 = 0;

    for (line_num, line) in lines.enumerate() {
        // Use FindStringSubmatchIndex to get positions within the line
        let Some(match_indices) = DIRECTIVE_PATTERN.captures(line) else {
            line_start_pos += line.len() as i64 + 1; // +1 for newline
            continue;
        };

        // matchIndices[0:2] is the full match range
        let full_match = match_indices.get(0).unwrap();
        let match_start = full_match.start() as i64;
        let match_end = full_match.end() as i64;

        // matchIndices[2:4] is the first capture group (-next-line or empty)
        let is_next_line = match_indices.get(1).is_some_and(|m| m.as_str() == "-next-line");

        // matchIndices[4:6] is the second capture group (rules string)
        let rules_str = match_indices.get(2).unwrap().as_str();

        let mut rules = Vec::new();
        for pair in RULE_SEVERITY_PATTERN.captures_iter(rules_str) {
            // Rust: Go's `len(pair) == 3` always holds (both groups always participate).
            rules.push(RuleSeverity {
                rule: pair[1].to_string(),
                severity: parse_severity(&pair[2]),
            });
        }

        if !rules.is_empty() {
            directives.push(Directive {
                line: line_num as i64,
                pos: line_start_pos + match_start,
                end: line_start_pos + match_end,
                is_next_line,
                rules,
            });
        }

        line_start_pos += line.len() as i64 + 1; // +1 for newline
    }

    directives
}

// go: internal/directives/parser.go:126 DirectiveSet a22ab3a39a65e39f
// DirectiveSet provides efficient lookup of directives by line and rule.
#[derive(Clone, Debug, Default)]
pub struct DirectiveSet {
    // byLine maps affected line number to directives affecting that line (for next-line directives)
    pub(crate) by_line: FxHashMap<i64, Vec<Directive>>,
    // fileLevel contains file-level suppressions (skip-file directives)
    pub(crate) file_level: Vec<RuleSeverity>,
    // sectionDirectives contains non-next-line, non-skip-file directives sorted by line
    // These apply from their position forward until the next section directive or end of file
    pub(crate) section_directives: Vec<Directive>,
    // usedDirectives tracks which directive lines have successfully suppressed a diagnostic
    // Key is the directive line number (not the affected line)
    pub(crate) used_directives: FxHashMap<i64, bool>,
}

// go: internal/directives/parser.go:140 BuildDirectiveSet 771bf423abe7eca6
// BuildDirectiveSet creates a DirectiveSet from a slice of directives for efficient lookup.
pub fn build_directive_set(directives: &[Directive]) -> DirectiveSet {
    let mut ds = DirectiveSet::default();

    for d in directives {
        let mut has_skip_file = false;
        // Check for file-level skip-file directives
        for rs in &d.rules {
            if rs.severity == Severity::SkipFile {
                ds.file_level.push(rs.clone());
                has_skip_file = true;
            }
        }

        if d.is_next_line {
            // Next-line directives affect a specific line
            let affected_line = d.affected_line();
            ds.by_line.entry(affected_line).or_default().push(d.clone());
        } else if !has_skip_file {
            // Section directives (non-next-line, non-skip-file) apply from their position forward
            // Filter out skip-file rules since they're already in fileLevel
            let mut section_rules = Vec::with_capacity(d.rules.len());
            for rs in &d.rules {
                if rs.severity != Severity::SkipFile {
                    section_rules.push(rs.clone());
                }
            }
            if !section_rules.is_empty() {
                let section_directive = Directive {
                    line: d.line,
                    is_next_line: false,
                    rules: section_rules,
                    ..Default::default()
                };
                ds.section_directives.push(section_directive);
            }
        }
    }

    ds
}

impl DirectiveSet {
    // go: internal/directives/parser.go:187 DirectiveSet.GetEffectiveSeverity 2889367aefa2b10d
    // GetEffectiveSeverity returns the effective severity for a rule at a given line.
    // It checks file-level suppressions first, then next-line directives, then section directives.
    // If no directive applies, it returns the provided default severity.
    // Rule matching is case-insensitive.
    pub fn get_effective_severity(
        &self,
        rule_name: &str,
        line: i64,
        default_severity: Severity,
    ) -> Severity {
        let rule_lower = go_strings_to_lower(rule_name);

        // Check file-level suppressions first (highest priority)
        for rs in &self.file_level {
            let rule_name_lower = go_strings_to_lower(&rs.rule);
            if rule_name_lower == rule_lower || rule_name_lower == "*" {
                return Severity::SkipFile;
            }
        }

        // Check next-line directives for this specific line
        if let Some(directives) = self.by_line.get(&line) {
            for d in directives {
                for rs in &d.rules {
                    let rule_name_lower = go_strings_to_lower(&rs.rule);
                    if rule_name_lower == rule_lower || rule_name_lower == "*" {
                        return rs.severity;
                    }
                }
            }
        }

        // Check section directives (find the most recent one for this rule before this line)
        for sd in self.section_directives.iter().rev() {
            if sd.line > line {
                continue;
            }
            for rs in &sd.rules {
                let rule_name_lower = go_strings_to_lower(&rs.rule);
                if rule_name_lower == rule_lower || rule_name_lower == "*" {
                    return rs.severity;
                }
            }
        }

        default_severity
    }

    // go: internal/directives/parser.go:228 DirectiveSet.IsSuppressed 9db00f2489e4c20d
    // IsSuppressed returns true if the given rule is suppressed (off or skip-file) at the given line.
    pub fn is_suppressed(&self, rule_name: &str, line: i64) -> bool {
        let severity = self.get_effective_severity(rule_name, line, Severity::Error);
        severity == Severity::Off || severity == Severity::SkipFile
    }

    // go: internal/directives/parser.go:235 DirectiveSet.IsSkipFile b47c87f1c4f2f3cc
    // IsSkipFile returns true if a skip-file directive applies to the given rule.
    // Unlike IsSuppressed, this only considers true file-level skip-file directives.
    pub fn is_skip_file(&self, rule_name: &str) -> bool {
        let rule_lower = go_strings_to_lower(rule_name);
        for rs in &self.file_level {
            let rule_name_lower = go_strings_to_lower(&rs.rule);
            if rule_name_lower == rule_lower || rule_name_lower == "*" {
                return true;
            }
        }
        false
    }

    // go: internal/directives/parser.go:248 DirectiveSet.GetEffectiveSeverityAndMarkUsed 992209bf7233e93e
    // GetEffectiveSeverityAndMarkUsed returns the effective severity and marks the directive as used
    // if it affected the result. This is used to track unused directives for warnings.
    pub fn get_effective_severity_and_mark_used(
        &mut self,
        rule_name: &str,
        line: i64,
        default_severity: Severity,
    ) -> Severity {
        let rule_lower = go_strings_to_lower(rule_name);

        // Check file-level suppressions first (highest priority)
        // File-level skip-file directives are never marked as "unused" since they affect the whole file
        for rs in &self.file_level {
            let rule_name_lower = go_strings_to_lower(&rs.rule);
            if rule_name_lower == rule_lower || rule_name_lower == "*" {
                return Severity::SkipFile;
            }
        }

        // Check next-line directives for this specific line
        if let Some(directives) = self.by_line.get(&line) {
            for d in directives {
                for rs in &d.rules {
                    let rule_name_lower = go_strings_to_lower(&rs.rule);
                    if rule_name_lower == rule_lower || rule_name_lower == "*" {
                        // Mark this directive as used
                        self.used_directives.insert(d.line, true);
                        return rs.severity;
                    }
                }
            }
        }

        // Check section directives (find the most recent one for this rule before this line)
        for sd in self.section_directives.iter().rev() {
            if sd.line > line {
                continue;
            }
            for rs in &sd.rules {
                let rule_name_lower = go_strings_to_lower(&rs.rule);
                if rule_name_lower == rule_lower || rule_name_lower == "*" {
                    // Section directives are not marked as unused since they affect multiple lines
                    return rs.severity;
                }
            }
        }

        default_severity
    }

    // go: internal/directives/parser.go:295 DirectiveSet.HasEnablingDirective 080c96b4b59b6e0b
    // HasEnablingDirective returns true if any file-level section directive or next-line directive
    // enables the given rule (i.e., sets it to a non-off, non-skip-file severity).
    // This is used to avoid skipping rules that are off by default but enabled via directives.
    pub fn has_enabling_directive(&self, rule_name: &str) -> bool {
        let rule_lower = go_strings_to_lower(rule_name);

        // Check section directives
        for sd in &self.section_directives {
            for rs in &sd.rules {
                let rule_name_lower = go_strings_to_lower(&rs.rule);
                if (rule_name_lower == rule_lower || rule_name_lower == "*")
                    && !rs.severity.is_off()
                {
                    return true;
                }
            }
        }

        // Check next-line directives
        for directives in self.by_line.values() {
            for d in directives {
                for rs in &d.rules {
                    let rule_name_lower = go_strings_to_lower(&rs.rule);
                    if (rule_name_lower == rule_lower || rule_name_lower == "*")
                        && !rs.severity.is_off()
                    {
                        return true;
                    }
                }
            }
        }

        false
    }

    // go: internal/directives/parser.go:330 DirectiveSet.HasAnyDirectiveForRule 9a8d520428fda437
    // HasAnyDirectiveForRule returns true if any directive in the file references
    // the given rule, either by name or via a wildcard, regardless of the severity
    // it assigns. This includes file-level, section, and next-line directives. It
    // is used to decide whether a rule below the minimum visible severity can be
    // safely skipped pre-execution: when a directive mentions the rule, running it
    // may be required either to honor a severity override or to mark the directive
    // as used for unusedDirective tracking.
    pub fn has_any_directive_for_rule(&self, rule_name: &str) -> bool {
        let rule_lower = go_strings_to_lower(rule_name);

        let matches = |rules: &[RuleSeverity]| -> bool {
            for rs in rules {
                let rule_name_lower = go_strings_to_lower(&rs.rule);
                if rule_name_lower == rule_lower || rule_name_lower == "*" {
                    return true;
                }
            }
            false
        };

        if matches(&self.file_level) {
            return true;
        }
        for sd in &self.section_directives {
            if matches(&sd.rules) {
                return true;
            }
        }
        for directives in self.by_line.values() {
            for d in directives {
                if matches(&d.rules) {
                    return true;
                }
            }
        }
        false
    }

    // go: internal/directives/parser.go:363 DirectiveSet.GetUnusedNextLineDirectives 4c639d87b242da16
    // GetUnusedNextLineDirectives returns next-line directives that did not suppress any diagnostic.
    // This is used to report unusedDirective warnings.
    pub fn get_unused_next_line_directives(&self, all_directives: &[Directive]) -> Vec<Directive> {
        let mut unused = Vec::new();
        for d in all_directives {
            if d.is_next_line && !self.used_directives.get(&d.line).copied().unwrap_or_default() {
                // Skip wildcard next-line directives from unused tracking since they apply to all rules.
                let mut has_wildcard = false;
                for rs in &d.rules {
                    if rs.rule == "*" {
                        has_wildcard = true;
                        break;
                    }
                }
                if has_wildcard {
                    continue;
                }
                unused.push(d.clone());
            }
        }
        unused
    }
}
