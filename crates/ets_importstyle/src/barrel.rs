use ets_shim::ast::{Kind, SourceFile, SourceFileParseOptions};
use ets_shim::compiler::{CompilerHost, Program};
use ets_shim::core::{P, get_script_kind_from_file_name, tspath};
use ets_shim::parser;
use rustc_hash::FxHashMap;

use crate::*;

// Rust: Go's `tspath.RootedFilePath` keys and arguments are plain file names (docs/TSRS_API.md).

// go: internal/importstyle/barrel.go:15 ProgramFiles
// ProgramFiles is the part of a program barrels are read from.
// *compiler.Program implements it.
pub trait ProgramFiles: Sync {
    fn get_source_file(&self, file_name: &str) -> Option<P<SourceFile>>;
    fn host(&self) -> &dyn CompilerHost;
}

/// Rust-only: Go's `*compiler.Program` satisfies `ProgramFiles` implicitly.
impl ProgramFiles for Program {
    fn get_source_file(&self, file_name: &str) -> Option<P<SourceFile>> {
        Program::get_source_file(self, file_name)
    }

    fn host(&self) -> &dyn CompilerHost {
        &**Program::host(self)
    }
}

// go: internal/importstyle/barrel.go:21 Barrel
// Barrel is the index module a module specifier should be imported through.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Barrel {
    // Specifier imports the barrel, e.g. "effect" or "effect/http".
    pub specifier: String,
    // ExportName is the name of the barrel's `export * as ExportName` that
    // forwards the module.
    pub export_name: String,
}

ets_go::impl_go_struct!(Barrel = "importstyle.Barrel" { "Specifier": specifier, "ExportName": export_name });

// go: internal/importstyle/barrel.go:31 BarrelReader
// BarrelReader finds barrels and caches what each index file forwards.
// It is not safe for concurrent use; keep one per checker or per request.
//
// Rust: `readFile`'s `(string, bool)` is `Option<String>`.
pub struct BarrelReader {
    // programFile returns an already parsed program file, or nil.
    pub(crate) program_file: Box<dyn Fn(&str) -> Option<P<SourceFile>> + Send + Sync>,
    pub(crate) read_file: Box<dyn Fn(&str) -> Option<String> + Send + Sync>,
    // forwards maps an index file to its forwarded modules, keyed by module
    // path without extension. A nil entry records a missing index file.
    pub(crate) forwards: FxHashMap<String, Option<FxHashMap<String, String>>>,
}

// go: internal/importstyle/barrel.go:40 NewBarrelReader
pub fn new_barrel_reader(
    program_file: impl Fn(&str) -> Option<P<SourceFile>> + Send + Sync + 'static,
    read_file: impl Fn(&str) -> Option<String> + Send + Sync + 'static,
) -> BarrelReader {
    BarrelReader {
        program_file: Box::new(program_file),
        read_file: Box::new(read_file),
        forwards: FxHashMap::default(),
    }
}

// go: internal/importstyle/barrel.go:48 NewProgramBarrelReader
// NewProgramBarrelReader reads barrels from the program, falling back to its file system.
pub fn new_program_barrel_reader(program: &'static dyn ProgramFiles) -> BarrelReader {
    new_barrel_reader(
        move |file_name| program.get_source_file(file_name),
        move |file_name| program.host().fs().read_file(file_name),
    )
}

// go: internal/importstyle/barrel.go:52 indexExtensions
pub(crate) static INDEX_EXTENSIONS: &[&str] =
    &[".d.ts", ".ts", ".tsx", ".d.mts", ".mts", ".d.cts", ".cts", ".js", ".mjs", ".cjs", ".jsx"];

// go: internal/importstyle/barrel.go:54 IsBarrelFile
pub fn is_barrel_file(file_name: &str) -> bool {
    file_stem(&tspath::get_base_file_name(file_name)) == "index"
}

impl BarrelReader {
    // go: internal/importstyle/barrel.go:63 BarrelReader.Find
    // Find walks up from the file a bare specifier resolves to, one directory and
    // one specifier segment at a time, and returns the nearest index module that
    // forwards it with `export * as Name`. Index files outside the program are
    // read from disk. It assumes the package's public subpaths mirror its
    // directory layout below the barrel, as `"./*"` style exports do.
    //
    // Rust: Go's nil receiver check is the caller's `Option<&mut BarrelReader>`.
    pub fn find(r: Option<&mut Self>, specifier: &str, module_file_name: &str) -> Option<Barrel> {
        let package_name = package_name(specifier);
        let r = r?;
        if package_name.is_empty()
            || module_file_name.is_empty()
            || is_barrel_file(module_file_name)
        {
            return None;
        }
        let module_path = tspath::remove_file_extension(module_file_name);
        let mut dir = tspath::get_directory_path(module_file_name);
        let mut spec = specifier;
        while spec.len() > package_name.len() {
            spec = &spec[..spec.rfind('/').unwrap()];
            for extension in INDEX_EXTENSIONS {
                let Some(forwards) = r
                    .barrel_forwards(&tspath::combine_paths(&dir, &[&format!("index{extension}")]))
                else {
                    continue;
                };
                if let Some(name) = forwards.get(module_path) {
                    return Some(Barrel { specifier: spec.to_string(), export_name: name.clone() });
                }
                break;
            }
            dir = tspath::get_directory_path(&dir);
        }
        None
    }

    // go: internal/importstyle/barrel.go:86 BarrelReader.barrelForwards
    // Rust: Go's `(forwards, forwards != nil)` is `Option<&map>`.
    pub(crate) fn barrel_forwards(
        &mut self,
        file_name: &str,
    ) -> Option<&FxHashMap<String, String>> {
        if !self.forwards.contains_key(file_name) {
            let mut forwards = None;
            if let Some(file) = self.parse_index_file(file_name) {
                forwards = Some(namespace_forwards(file));
            }
            self.forwards.insert(file_name.to_string(), forwards);
        }
        self.forwards[file_name].as_ref()
    }

    // go: internal/importstyle/barrel.go:98 BarrelReader.parseIndexFile
    pub(crate) fn parse_index_file(&self, file_name: &str) -> Option<P<SourceFile>> {
        if let Some(file) = (self.program_file)(file_name) {
            return Some(file);
        }
        let text = (self.read_file)(file_name)?;
        Some(parser::parse_source_file(
            SourceFileParseOptions { file_name: file_name.to_string(), ..Default::default() },
            &text,
            get_script_kind_from_file_name(file_name),
        ))
    }
}

// go: internal/importstyle/barrel.go:111 namespaceForwards
// namespaceForwards collects the barrel's relative `export * as Name` statements.
// The result is never nil, so an index file without any marks a found barrel.
pub(crate) fn namespace_forwards(barrel: P<SourceFile>) -> FxHashMap<String, String> {
    let mut forwards = FxHashMap::default();
    let dir = tspath::get_directory_path(barrel.file_name());
    for statement in barrel.statements().nodes() {
        if statement.kind() != Kind::ExportDeclaration {
            continue;
        }
        let declaration = statement.as_export_declaration();
        let (Some(export_clause), Some(module_specifier)) =
            (declaration.export_clause, declaration.module_specifier)
        else {
            continue;
        };
        if export_clause.kind() != Kind::NamespaceExport
            || module_specifier.kind() != Kind::StringLiteral
        {
            continue;
        }
        let relative = module_specifier.text();
        if !relative.starts_with("./") && !relative.starts_with("../") {
            continue;
        }
        let name = export_clause.as_namespace_export().name;
        if name.kind() != Kind::Identifier {
            continue;
        }
        forwards.insert(
            tspath::remove_file_extension(&tspath::get_normalized_absolute_path(relative, &dir))
                .to_string(),
            name.text().to_string(),
        );
    }
    forwards
}

// go: internal/importstyle/barrel.go:136 fileStem
pub(crate) fn file_stem(base: &str) -> &str {
    for extension in DECLARATION_EXTENSIONS {
        if let Some(trimmed) = base.strip_suffix(extension) {
            return trimmed;
        }
    }
    base
}
