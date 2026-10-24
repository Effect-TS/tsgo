// Port of etscheckerhooks's external test package (`package etscheckerhooks_test`): one module per Go file.
//
// Rust: the Go package imports `etscheckerhooks` for its `init`, which registers the checker hooks: the tests call
// `ets_etscheckerhooks::init()` before building a program or a language server.

mod external_library_test;
mod unused_parameters_test;
