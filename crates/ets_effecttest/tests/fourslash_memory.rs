//! Rust-only: a regression guard for the memory of the fourslash-based runners (no Go counterpart: Go's GC frees a
//! finished test's language server). Every quick-fix, refactor, completion, ... case runs an in-process tsrs language
//! server (`ets_fourslash`), and a server that outlives its case keeps its session, snapshots and programs alive:
//! `TestEffectQuickFixes` once grew by ~60 MB per case (`_patches/tsrs/102-lsp-session-client-weak.patch`).
//!
//! A test binary of its own, with a single test, so that no other test allocates while it reads the process's RSS.

use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use ets_bundledeffect::EffectVersion;

/// The cases repeated by the RSS check, and how many times.
const CASES: usize = 4;
const ROUNDS: usize = 6;
/// The allowed RSS growth per case run after the first round (the leak was ~60 MB per case; a freed case leaves a
/// few hundred KB in the shared parse cache and the allocator).
const MAX_GROWTH_PER_RUN: u64 = 8 << 20;

/// The process's resident set size (`VmRSS` of `/proc/self/status`), `None` where there is no procfs.
fn vm_rss() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kb: u64 =
        line.trim_start_matches("VmRSS:").trim().trim_end_matches("kB").trim().parse().ok()?;
    Some(kb << 10)
}

/// Waits until nothing holds `weak` any more (the server's read thread is not joined and may hold it a moment longer).
fn wait_freed<T: ?Sized>(weak: &Weak<T>) -> bool {
    let deadline = Instant::now() + Duration::from_secs(30);
    while weak.strong_count() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    weak.strong_count() == 0
}

#[test]
fn fourslash_cases_free_their_language_server() {
    ets_go::testing::on_checker_stack(|| {
        ets_go::testing::run(|t| {
            if let Err(err) =
                ets_bundledeffect::ensure_package_installed(EffectVersion::V4, "effect")
            {
                t.skip(format!("Effect not installed: {err}"));
            }
            ets_effecttest::init_hooks();

            // The server and its session are freed when the test ends.
            let mut f = ets_fourslash::new_fourslash(
                t,
                None,
                "// @filename: /tsconfig.json\n{}\n// @filename: /a.ts\nexport const a = 1;\n",
            );
            let server = Arc::downgrade(&f.client().server);
            let session = Arc::downgrade(f.session());
            f.done(t);
            drop(f);
            assert!(wait_freed(&server), "the language server outlives its fourslash test");
            assert!(wait_freed(&session), "the session outlives its fourslash test");

            // Running cases again and again does not grow the process.
            let cases = ets_effecttest::discover_test_cases(EffectVersion::V4)
                .unwrap_or_else(|err| t.fatal(format!("Failed to discover test cases: {err}")));
            let cases = &cases[..CASES.min(cases.len())];
            let mut after_first_round = None;
            for _ in 0..ROUNDS {
                for tc in cases {
                    ets_effecttest::run_effect_quick_fix_test(t, EffectVersion::V4, tc);
                }
                after_first_round = after_first_round.or_else(vm_rss);
            }
            let (Some(before), Some(after)) = (after_first_round, vm_rss()) else {
                return; // no procfs
            };
            let runs = ((ROUNDS - 1) * cases.len()) as u64;
            let growth = after.saturating_sub(before);
            println!(
                "RSS after the first round: {} MB, after {runs} more runs: {} MB",
                before >> 20,
                after >> 20
            );
            assert!(
                growth <= MAX_GROWTH_PER_RUN * runs,
                "RSS grew by {} MB over {runs} fourslash case runs ({} MB to {} MB); a finished case is not freed",
                growth >> 20,
                before >> 20,
                after >> 20
            );
        });
    });
}
