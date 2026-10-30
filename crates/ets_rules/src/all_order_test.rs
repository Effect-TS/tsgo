//! Rust-only: `ALL` holds the ported rules in Go's `All` order (porting/PORTING.md, "Adding a rule to `ets_rules`").

use crate::*;

/// The rule names of Go's `var All = []rule.Rule{...}` in `internal/rules/rules.go`, in order. Every rule's `Name`
/// is its Go identifier with the first letter lowercased.
fn go_all_names() -> Vec<String> {
    let path = ets_bundledeffect::effect_ts_go_root_path().join("internal/rules/rules.go");
    let source = std::fs::read_to_string(&path).expect("read internal/rules/rules.go");
    let start = source.find("var All = []rule.Rule{").expect("var All in rules.go");
    let block = &source[start..];
    let block = &block[..block.find("\n}").expect("end of var All")];
    block
        .lines()
        .skip(1)
        .filter_map(|line| {
            let ident = line.trim().strip_suffix(',')?;
            let mut chars = ident.chars();
            let first = chars.next()?;
            Some(first.to_ascii_lowercase().to_string() + chars.as_str())
        })
        .collect()
}

#[test]
fn all_follows_go_order() {
    let go_names = go_all_names();
    let mut next = 0;
    for rule in ALL.iter() {
        let Some(index) = go_names[next..].iter().position(|name| name == rule.name) else {
            panic!(
                "rule {:?} is not in Go's All after the previous ported rule (wrong position, or not in Go's All)",
                rule.name
            );
        };
        next += index + 1;
    }
}

#[test]
fn all_is_go_all() {
    let go_names = go_all_names();
    let names: Vec<&str> = ALL.iter().map(|rule| rule.name).collect();
    let want: Vec<&str> = go_names.iter().map(String::as_str).collect();
    assert_eq!(names, want, "ALL must be exactly Go's All, in order");
}
