// Package typeparser provides Effect type detection and parsing utilities.

use ets_shim::ast::{SourceFile, SourceFileMetaData};
use ets_shim::compiler::Program;
use ets_shim::core::P;
use ets_shim::core::tspath::Path;
use ets_shim::module::packagejson::{InfoCacheEntry, PackageJson};

use crate::*;

// go: internal/typeparser/packagejson.go:10 packageJsonProgram ad8c5cb98911ab68
// Rust: Go's `tspath.PathKey` and `tspath.RootedFilePath` are tsrs's `Path` and a plain file name (porting/TSRS_API.md).
pub(crate) trait PackageJsonProgram {
    fn get_source_file_meta_data(&self, path: &Path) -> SourceFileMetaData;
    fn get_package_json_info(&self, pkg_json_path: &str) -> Option<P<InfoCacheEntry>>;
}

// go: internal/typeparser/packagejson.go:10 packageJsonProgram ad8c5cb98911ab68
impl PackageJsonProgram for Program {
    fn get_source_file_meta_data(&self, path: &Path) -> SourceFileMetaData {
        Program::get_source_file_meta_data(self, path).clone()
    }

    fn get_package_json_info(&self, pkg_json_path: &str) -> Option<P<InfoCacheEntry>> {
        Program::get_package_json_info(self, pkg_json_path)
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/packagejson.go:17 TypeParser.PackageJsonForSourceFile a9b87ffbe19bd8bb
    // PackageJsonForSourceFile returns the nearest package.json contents for a source file, or nil.
    // Results are cached per source file on EffectLinks for the checker's lifetime.
    pub fn package_json_for_source_file(
        &mut self,
        sf: impl Into<Option<P<SourceFile>>>,
    ) -> Option<P<PackageJson>> {
        let sf = sf.into()?;

        cached(
            self,
            |l| &mut l.package_json_for_source_file,
            sf,
            |tp| {
                // Rust: Go's assertion to packageJsonProgram succeeds for the compiler program (see `TypeParser`).
                let prog: &dyn PackageJsonProgram = tp.program;

                let meta = prog.get_source_file_meta_data(sf.path());
                if meta.package_json_directory.is_empty() {
                    return None;
                }

                let package_json_path = ets_shim::core::tspath::combine_paths(
                    &meta.package_json_directory,
                    &["package.json"],
                );
                let info = prog.get_package_json_info(&package_json_path)?;
                info.get_contents()
            },
        )
    }
}
