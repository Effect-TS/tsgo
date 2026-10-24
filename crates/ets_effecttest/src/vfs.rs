// Package effecttest provides test utilities for Effect diagnostic tests.

use std::fmt::Display;
use std::sync::{Arc, LazyLock};

use ets_bundledeffect::EffectVersion;
use ets_shim::ast::{SourceFile, SourceFileParseOptions};
use ets_shim::compiler::CompilerHost;
use ets_shim::contentmapper::{self, Mapper, Project};
use ets_shim::core::P;
use ets_shim::core::arena::Region;
use ets_shim::core::tspath::Path;
use ets_shim::diagnostics::Message;
use ets_shim::tsoptions::ParsedCommandLine;
use ets_shim::vfs::{FS, bundled};
use parking_lot::{Condvar, Mutex};
use rustc_hash::FxHashMap;

// go test: internal/effecttest/vfs.go:17 programSemaphore
// programSemaphore limits the number of concurrent TypeScript program
// compilations to avoid OOM in memory-constrained environments.
//
// Rust: Go's buffered channel; a count of the slots in use under a mutex.
pub(crate) static PROGRAM_SEMAPHORE: LazyLock<(Mutex<usize>, Condvar)> =
    LazyLock::new(|| (Mutex::new(0), Condvar::new()));

// go test: internal/effecttest/vfs.go:19 maxConcurrentPrograms
// Rust: `runtime.GOMAXPROCS(0)` is the number of CPUs by default.
pub(crate) fn max_concurrent_programs() -> usize {
    let n = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    n.max(1)
}

// go test: internal/effecttest/vfs.go:26 AcquireProgram
// AcquireProgram acquires a slot from the program semaphore.
pub fn acquire_program() {
    let (used, freed) = &*PROGRAM_SEMAPHORE;
    let mut used = used.lock();
    while *used >= max_concurrent_programs() {
        freed.wait(&mut used);
    }
    *used += 1;
}

// go test: internal/effecttest/vfs.go:30 ReleaseProgram
// ReleaseProgram releases a slot back to the program semaphore, clears
// per-program caches to allow GC of the program, and triggers GC.
//
// Rust: there is no GC to trigger; `run_effect_test` frees its program itself (`release_test_program`).
pub fn release_program() {
    let (used, freed) = &*PROGRAM_SEMAPHORE;
    *used.lock() -= 1;
    freed.notify_one();
}

/// Rust-only: Go's `defer ReleaseProgram()`, so that a test that stops early (`t.Fatal`) gives its slot back.
pub(crate) struct ReleaseProgramOnDrop;

impl Drop for ReleaseProgramOnDrop {
    fn drop(&mut self) {
        release_program();
    }
}

// go test: internal/effecttest/vfs.go:37 astCacheKey
// astCacheKey combines EffectVersion and filename to avoid cross-version
// collisions (V3 and V4 mount different package contents at the same VFS paths).
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct AstCacheKey {
    pub(crate) version: EffectVersion,
    pub(crate) file_name: String,
}

/// Rust-only: a cached source file and the region it was parsed and bound in. Go's GC keeps a cached AST alive
/// while the cache refers to it; here the file lives in a region of its own (as in `tsrs_project`'s parse cache), so
/// that freeing a program's regions leaves the files other programs share intact. Entries are never removed, as in
/// Go.
pub(crate) struct CachedSourceFile {
    pub(crate) file: P<SourceFile>,
    pub(crate) _region: Region,
}

// go test: internal/effecttest/vfs.go:47 parsedASTCache
// parsedASTCache caches parsed ASTs for node_modules package files.
// These files are identical across all tests of the same EffectVersion,
// so parsing them once and reusing the ASTs across programs saves
// significant memory and CPU. The cache is bounded by the number of
// unique package files per version (finite).
pub(crate) static PARSED_AST_CACHE: LazyLock<Mutex<FxHashMap<AstCacheKey, CachedSourceFile>>> =
    LazyLock::new(Mutex::default);

// go test: internal/effecttest/vfs.go:52 parsedLibCache
// parsedLibCache caches parsed ASTs for bundled lib files (e.g. lib.es2022.d.ts).
// Lib files are version-independent and identical across all tests, so they
// are keyed by filename only. There are ~108 lib files, so this cache is bounded.
pub(crate) static PARSED_LIB_CACHE: LazyLock<Mutex<FxHashMap<String, CachedSourceFile>>> =
    LazyLock::new(Mutex::default);

// go test: internal/effecttest/vfs.go:57 cachingCompilerHost
// cachingCompilerHost wraps a CompilerHost and caches GetSourceFile results
// for files under /node_modules/ and bundled lib files. Test-specific files
// are not cached.
//
// Rust: Go embeds the wrapped host; the trait's other methods forward to it.
pub(crate) struct CachingCompilerHost {
    pub(crate) compiler_host: Arc<dyn CompilerHost>,
    pub(crate) version: EffectVersion,
}

impl CachingCompilerHost {
    /// Rust-only: `h.CompilerHost.GetSourceFile(opts)` for a file the cache keeps: parsed and bound in a region of
    /// its own (see `CachedSourceFile`; sized as `tsrs_project`'s parse cache sizes its file regions).
    fn parse_shared(&self, opts: SourceFileParseOptions) -> Option<CachedSourceFile> {
        let text_len =
            self.compiler_host.fs().read_file(&opts.file_name).map_or(0, |text| text.len());
        let region = Region::new(text_len.saturating_mul(10).saturating_add(4 << 10).min(64 << 20));
        let file = {
            let _scope = region.enter();
            let file = self.compiler_host.get_source_file(opts)?;
            ets_shim::binder::bind_source_file(file);
            file
        };
        Some(CachedSourceFile { file, _region: region })
    }
}

impl CompilerHost for CachingCompilerHost {
    fn fs(&self) -> &dyn FS {
        self.compiler_host.fs()
    }

    fn default_library_path(&self) -> &str {
        self.compiler_host.default_library_path()
    }

    fn get_current_directory(&self) -> &str {
        self.compiler_host.get_current_directory()
    }

    fn trace(&self, msg: &'static Message, args: &[&dyn Display]) {
        self.compiler_host.trace(msg, args);
    }

    // go test: internal/effecttest/vfs.go:62 cachingCompilerHost.GetSourceFile
    //
    // Rust: Go's `sync.Map` lets two tests that miss at once both parse and store (the later store wins); here the
    // first stored file wins and both return it, so a returned file is always the one the cache keeps alive.
    fn get_source_file(&self, opts: SourceFileParseOptions) -> Option<P<SourceFile>> {
        // Cache bundled lib files (version-independent, keyed by filename only)
        if bundled::is_bundled(&opts.file_name) {
            if let Some(cached) = PARSED_LIB_CACHE.lock().get(&opts.file_name) {
                return Some(cached.file);
            }
            let file_name = opts.file_name.clone();
            let sf = self.parse_shared(opts)?;
            return Some(PARSED_LIB_CACHE.lock().entry(file_name).or_insert(sf).file);
        }
        // Cache package files (under /node_modules/, keyed by version + filename)
        if opts.file_name.starts_with("/node_modules/") {
            let key = AstCacheKey { version: self.version, file_name: opts.file_name.clone() };
            if let Some(cached) = PARSED_AST_CACHE.lock().get(&key) {
                return Some(cached.file);
            }
            let sf = self.parse_shared(opts)?;
            return Some(PARSED_AST_CACHE.lock().entry(key).or_insert(sf).file);
        }
        self.compiler_host.get_source_file(opts)
    }

    fn get_content_mapped_source_files(
        &self,
        parse_options: SourceFileParseOptions,
        mapper: &Mapper,
    ) -> Result<contentmapper::SourceFiles, contentmapper::Error> {
        self.compiler_host.get_content_mapped_source_files(parse_options, mapper)
    }

    fn content_mapper_project(&self) -> Option<Arc<dyn Project>> {
        self.compiler_host.content_mapper_project()
    }

    fn get_resolved_project_reference(
        &self,
        file_name: &str,
        path: Path,
    ) -> Option<P<ParsedCommandLine>> {
        self.compiler_host.get_resolved_project_reference(file_name, path)
    }
}
