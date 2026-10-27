// Package importstyle classifies module specifiers against the configured
// namespaceImportPackages and barrelImportPackages options.

use ets_etscore::ResolvedEffectPluginOptions;
use ets_shim::core::stringutil::equal_fold;

// go: internal/importstyle/importstyle.go:11 Style 815b17f01a44d825
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Style {
    // go: internal/importstyle/importstyle.go:13 StyleNone cc93ce6039ab3baf
    #[default]
    None,
    Namespace,
    Barrel,
}

/// Rust-only: Go's `%d` of a `Style` (a `uint8`).
impl ets_go::fmt::Value for Style {
    fn go_value(&self) -> ets_go::fmt::GoValue<'_> {
        ets_go::fmt::GoValue::Uint(
            *self as u64,
            "importstyle.Style".into(),
            ets_go::fmt::Kind::Uint8,
        )
    }
}

// go: internal/importstyle/importstyle.go:21 Policy 8534b2c6775f8bb5
// Policy holds the configured package lists. It references the option slices
// directly, so building one per file costs nothing.
//
// Rust: it owns copies of the two lists, so a policy can outlive the options it was built from (autoimportstyle
// keeps one in a fix transformer); the lists are short.
#[derive(Clone, Debug, Default)]
pub struct Policy {
    pub(crate) namespace_packages: Vec<String>,
    pub(crate) barrel_packages: Vec<String>,
}

// go: internal/importstyle/importstyle.go:26 NewPolicy b0ddf5fe0f9490c7
pub fn new_policy(options: Option<&ResolvedEffectPluginOptions>) -> Policy {
    let Some(options) = options else {
        return Policy::default();
    };
    Policy {
        namespace_packages: options.namespace_import_packages.clone(),
        barrel_packages: options.barrel_import_packages.clone(),
    }
}

impl Policy {
    // go: internal/importstyle/importstyle.go:36 Policy.IsEmpty 4179d38d986a9c6e
    pub fn is_empty(&self) -> bool {
        self.namespace_packages.is_empty() && self.barrel_packages.is_empty()
    }

    // go: internal/importstyle/importstyle.go:42 Policy.PackageStyle 4e07588cfb889cec
    // PackageStyle returns the style configured for a package name. Matching is
    // case-insensitive, and a package listed in both options uses namespace imports.
    pub fn package_style(&self, package_name: &str) -> Style {
        if package_name.is_empty() {
            return Style::None;
        }
        if contains_fold(&self.namespace_packages, package_name) {
            return Style::Namespace;
        }
        if contains_fold(&self.barrel_packages, package_name) {
            return Style::Barrel;
        }
        Style::None
    }

    // go: internal/importstyle/importstyle.go:55 Policy.Style 8c3fe8a23af915af
    pub fn style(&self, specifier: &str) -> Style {
        self.package_style(package_name(specifier))
    }
}

// go: internal/importstyle/importstyle.go:59 containsFold a98160da444816a4
pub(crate) fn contains_fold(packages: &[String], package_name: &str) -> bool {
    for pkg in packages {
        if equal_fold(pkg, package_name) {
            return true;
        }
    }
    false
}

// go: internal/importstyle/importstyle.go:72 PackageName ebc5b13e49c98407
// PackageName extracts the npm package name from a bare module specifier.
// For scoped packages like "@scope/pkg/sub", returns "@scope/pkg".
// For non-scoped packages like "pkg/sub", returns "pkg".
// For relative and absolute paths, returns "".
pub fn package_name(specifier: &str) -> &str {
    if specifier.is_empty() || specifier.as_bytes()[0] == b'.' || specifier.as_bytes()[0] == b'/' {
        return "";
    }
    if specifier.as_bytes()[0] == b'@' {
        let Some(slash_idx) = specifier.find('/') else {
            return specifier;
        };
        let Some(second_slash) = specifier[slash_idx + 1..].find('/') else {
            return specifier;
        };
        return &specifier[..slash_idx + 1 + second_slash];
    }
    let (name, _) = specifier.split_once('/').unwrap_or((specifier, ""));
    name
}

// go: internal/importstyle/importstyle.go:93 NamespaceName 6abd44afaac9692c
// NamespaceName derives a namespace name from the last segment of a module specifier.
// For example, "effect/Effect" -> "Effect", "effect" -> "effect", "@scope/pkg/Foo" -> "Foo".
pub fn namespace_name(specifier: &str) -> &str {
    &specifier[specifier.rfind('/').map_or(0, |i| i + 1)..]
}

// go: internal/importstyle/importstyle.go:97 declarationExtensions 8ffe86186d0de76f
pub(crate) static DECLARATION_EXTENSIONS: &[&str] =
    &[".d.ts", ".d.mts", ".d.cts", ".ts", ".mts", ".cts", ".js", ".mjs", ".cjs"];

// go: internal/importstyle/importstyle.go:103 ReexportSpecifier 7e324e6bf10b715a
// ReexportSpecifier maps a sibling re-export inside a barrel ("./Effect.ts" in
// the index of "effect") to its public specifier ("effect/Effect"). It returns
// "" when the target is not a direct sibling, since only siblings map onto the
// package's subpath exports one to one.
pub fn reexport_specifier(barrel: &str, relative: &str) -> String {
    let Some(mut name) = relative.strip_prefix("./") else {
        return String::new();
    };
    if barrel.is_empty() {
        return String::new();
    }
    for extension in DECLARATION_EXTENSIONS {
        if let Some(trimmed) = name.strip_suffix(extension) {
            name = trimmed;
            break;
        }
    }
    if name.is_empty() || name == "index" || name.contains(['/', '.']) {
        return String::new();
    }
    format!("{barrel}/{name}")
}
