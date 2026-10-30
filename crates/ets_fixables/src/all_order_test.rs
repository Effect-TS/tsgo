//! Rust-only: `ALL` is exactly Go's `All`, in order (porting/PORTING.md, "Porting a fixable, refactor or
//! completion").

use crate::*;

/// The `Name`s of the fixables of Go's `var All = []fixable.Fixable{...}` in `internal/fixables/fixables.go`, in
/// order: each identifier of the list is looked up as `var <Ident> = fixable.Fixable{` in the package's files.
fn go_all_names() -> Vec<String> {
    let dir = ets_bundledeffect::effect_ts_go_root_path().join("internal/fixables");
    let source = std::fs::read_to_string(dir.join("fixables.go")).expect("read fixables.go");
    let start = source.find("var All = []fixable.Fixable{").expect("var All in fixables.go");
    let block = &source[start..];
    let block = &block[..block.find("\n}").expect("end of var All")];
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("read internal/fixables") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|ext| ext == "go") {
            files.push(std::fs::read_to_string(&path).expect("read Go file"));
        }
    }
    block
        .lines()
        .skip(1)
        .filter_map(|line| line.trim().strip_suffix(','))
        .filter(|ident| !ident.starts_with("//"))
        .map(|ident| {
            let decl = format!("var {ident} = fixable.Fixable{{");
            let file = files
                .iter()
                .find_map(|f| f.find(&decl).map(|at| &f[at..]))
                .unwrap_or_else(|| panic!("declaration of {ident}"));
            let name = &file[file.find("Name:").expect("Name field") + "Name:".len()..];
            let name = &name[name.find('"').unwrap() + 1..];
            name[..name.find('"').unwrap()].to_string()
        })
        .collect()
}

#[test]
fn all_follows_go_order() {
    let go_names = go_all_names();
    let mut next = 0;
    for fixable in ALL.iter() {
        let Some(index) = go_names[next..].iter().position(|name| name == fixable.name) else {
            panic!(
                "fixable {:?} is not in Go's All after the previous ported fixable (wrong position, or not in Go's All)",
                fixable.name
            );
        };
        next += index + 1;
    }
}

#[test]
fn all_is_go_all() {
    let go_names = go_all_names();
    let names: Vec<&str> = ALL.iter().map(|fixable| fixable.name).collect();
    let want: Vec<&str> = go_names.iter().map(String::as_str).collect();
    assert_eq!(names, want, "ALL must be exactly Go's All, in order");
}
