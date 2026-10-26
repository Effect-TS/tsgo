//! Rust-only: the effect-tsgo executable on tsrs, the counterpart of the patched `cmd/tsc/main.go`
//! (`_patches/typescript/001-cmd-tsgo-main.patch`).
//!
//! Go registers the Effect hooks with blank imports, whose `init()`s run before `main`:
//!
//! ```go
//! _ "github.com/effect-ts/tsgo/etscheckerhooks" // checker diagnostics hooks
//! "github.com/effect-ts/tsgo/etsdiagnostics"
//! _ "github.com/effect-ts/tsgo/etsexecutehooks" // exit-code filtering hooks
//! _ "github.com/effect-ts/tsgo/etslshooks"      // LS codefix hooks
//! ```
//!
//! and adds `--effect-cli-diagnostics` (`etsdiagnostics.Run`) to the commands. Rust has no `init()`: `register_hooks`
//! calls each hook crate's `pub fn init()` in that order; `--effect-cli-diagnostics` runs `ets_etsdiagnostics::run`,
//! anything else runs tsrs's CLI (`tsrs_cli::main`, which
//! dispatches `--lsp`, `--api` and the command line as Go's `runMain` does).

/// Rust-only: Go's `init()`s of the hook packages `cmd/tsc/main.go` imports, in import order.
fn register_hooks() {
    ets_etscheckerhooks::init();
    ets_etsexecutehooks::init();
    ets_etslshooks::init();
}

/// Rust-only: Go's `case "--effect-cli-diagnostics": return etsdiagnostics.Run(context.Background(), args[1:],
/// os.Stdout, os.Stderr)` in `runMain`, before tsrs's CLI sees the arguments. Like tsrs's command line, the request
/// runs on a thread with a large stack (checking recurses deeply).
fn run_effect_cli_diagnostics() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("--effect-cli-diagnostics") {
        return;
    }
    let rest = args[1..].to_vec();
    let status = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || {
            let ctx = ets_shim::core::context::Context::background();
            ets_etsdiagnostics::run(&ctx, &rest, &mut std::io::stdout(), &mut std::io::stderr())
        })
        .unwrap()
        .join()
        .unwrap_or(1);
    std::process::exit(status);
}

#[cfg(feature = "cli")]
fn main() -> std::process::ExitCode {
    register_hooks();
    run_effect_cli_diagnostics();
    tsrs_cli::main()
}

/// Rust-only: without the `cli` feature (see Cargo.toml) there is no compiler to run.
#[cfg(not(feature = "cli"))]
fn main() {
    register_hooks();
    run_effect_cli_diagnostics();
    eprintln!("tsrs: built without the `cli` feature (cargo build -p ets_tsrs --features cli)");
    std::process::exit(1);
}
