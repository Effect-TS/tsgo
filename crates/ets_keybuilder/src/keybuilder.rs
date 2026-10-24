use ets_etscore::KeyPattern;
use ets_shim::P;
use ets_shim::ast::SourceFile;
use ets_shim::checker::Program;
use ets_shim::core::stringutil::{equal_fold, go_strings_to_lower};
use ets_shim::core::tspath;
use ets_typeparser::TypeParser;

// go: internal/keybuilder/keybuilder.go:17 Cyrb53
// Cyrb53 computes a fast non-cryptographic hash of the input string,
// producing a 16-character zero-padded hex string from two uint32 halves.
// This is a Go port of the cyrb53 function from the reference TS implementation.
pub fn cyrb53(str: &str) -> String {
    let mut h1: u32 = 0xdead_beef;
    let mut h2: u32 = 0x41c6_ce57;

    for &b in str.as_bytes() {
        let ch = b as u32;
        h1 = imul(h1 ^ ch, 2_654_435_761);
        h2 = imul(h2 ^ ch, 1_597_334_677);
    }

    h1 = imul(h1 ^ (h1 >> 16), 2_246_822_507);
    h1 ^= imul(h2 ^ (h2 >> 13), 3_266_489_909);
    h2 = imul(h2 ^ (h2 >> 16), 2_246_822_507);
    h2 ^= imul(h1 ^ (h1 >> 13), 3_266_489_909);

    format!("{h2:08x}{h1:08x}")
}

// go: internal/keybuilder/keybuilder.go:37 imul
// imul emulates JavaScript's Math.imul: 32-bit integer multiplication
// that discards overflow (keeps only the lower 32 bits).
pub(crate) fn imul(a: u32, b: u32) -> u32 {
    (a as u64 * b as u64) as u32
}

// go: internal/keybuilder/keybuilder.go:45 CreateString
// CreateString computes the expected key string for a class declaration.
// It takes the source file name, package name, package directory, class name,
// and target category, and returns the expected key string using the first
// matching key pattern. Returns empty string if no pattern matches.
pub fn create_string(
    source_file_name: &str,
    package_name: &str,
    package_directory: &str,
    class_name: &str,
    target: &str,
    key_patterns: &[KeyPattern],
) -> String {
    for key_pattern in key_patterns {
        if key_pattern.target != target {
            continue;
        }

        // Construct onlyFileName: basename without extension, strip "index" suffix
        let mut only_file_name: &str = match source_file_name.rfind('/') {
            None => source_file_name,
            Some(last_index) => &source_file_name[last_index + 1..],
        };
        if let Some(last_ext_index) = only_file_name.rfind('.') {
            only_file_name = &only_file_name[..last_ext_index];
        }
        if go_strings_to_lower(only_file_name).ends_with("/index") {
            only_file_name = &only_file_name[..only_file_name.len() - 6];
        }
        // The TS reference strips "index" when the filename IS "index" (after removing extension)
        if go_strings_to_lower(only_file_name) == "index" {
            only_file_name = "";
        }
        only_file_name = only_file_name.strip_prefix('/').unwrap_or(only_file_name);

        // Construct subDirectory: directory relative to package directory
        let mut sub_directory = tspath::get_directory_path(source_file_name);
        if !sub_directory.starts_with(package_directory) {
            continue;
        }
        sub_directory = sub_directory[package_directory.len()..].to_string();
        if !sub_directory.ends_with('/') {
            sub_directory += "/";
        }
        let mut sub_directory: &str = sub_directory.strip_prefix('/').unwrap_or(&sub_directory);
        for prefix in key_pattern.skip_leading_path.as_deref().unwrap_or_default() {
            if let Some(rest) = sub_directory.strip_prefix(prefix.as_str()) {
                sub_directory = rest;
                break;
            }
        }

        // Construct parts based on pattern
        let mut parts: Vec<&str>;
        let class_name_matches = equal_fold(only_file_name, class_name);

        match key_pattern.pattern.as_str() {
            "package-identifier" => {
                parts = vec![package_name, only_file_name];
                if !class_name_matches {
                    parts.push(class_name);
                }
            }
            "identifier" => {
                parts = vec![class_name];
            }
            _ => {
                // "default" and "default-hashed"
                parts = vec![package_name, sub_directory, only_file_name];
                if !class_name_matches {
                    parts.push(class_name);
                }
            }
        }

        // Strip leading/trailing slashes from each part
        for part in &mut parts {
            let mut p: &str = part;
            p = p.strip_prefix('/').unwrap_or(p);
            p = p.strip_suffix('/').unwrap_or(p);
            *part = p;
        }

        // Filter empty parts and join
        let mut filtered: Vec<&str> = Vec::new();
        for part in parts {
            if !ets_go::strings::trim_space(part).is_empty() {
                filtered.push(part);
            }
        }
        let full_key = filtered.join("/");

        // Hash if default-hashed pattern
        if key_pattern.pattern == "default-hashed" {
            return cyrb53(&full_key);
        }
        return full_key;
    }

    String::new()
}

// go: internal/keybuilder/keybuilder.go:133 CreateStringForSourceFile
pub fn create_string_for_source_file(
    program: &'static dyn Program,
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    class_name: &str,
    target: &str,
    key_patterns: &[KeyPattern],
) -> String {
    let mut package_name = String::new();
    if let Some(pkg_json) = tp.package_json_for_source_file(sf) {
        package_name = pkg_json.name.get_value().cloned().unwrap_or_default();
    }

    let root_directory = key_root_directory(program, sf);
    if root_directory.is_empty() {
        return String::new();
    }

    create_string(sf.file_name(), &package_name, &root_directory, class_name, target, key_patterns)
}

// go: internal/keybuilder/keybuilder.go:147 keyRootDirectory
pub(crate) fn key_root_directory(program: &'static dyn Program, sf: P<SourceFile>) -> String {
    let package_directory = program.get_source_file_meta_data(sf.path()).package_json_directory;
    if !package_directory.is_empty() {
        return package_directory;
    }
    // The tsconfig directory comes before the common source directory because
    // it does not move as files are added to the program.
    let config_file_path = program.options().config_file_path.clone();
    if !config_file_path.is_empty() {
        return tspath::get_directory_path(&config_file_path);
    }
    program.common_source_directory()
}
