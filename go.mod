module github.com/effect-ts/tsgo

go 1.26

require (
	github.com/effect-ts/tsgo/etscore v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/api v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/ast v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/astnav v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/bundled v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/checker v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/compiler v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/core v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/diagnostics v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/evaluator v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/execute/tsc v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/fourslash v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/jsnum v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/locale v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/ls v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/ls/autoimport v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/ls/change v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/ls/lsconv v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/ls/lsutil v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/module v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/modulespecifiers v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/packagejson v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/parser v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/project v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/project/logging v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/scanner v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/sourcemap v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/testutil/baseline v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/testutil/harnessutil v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/tsoptions v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/tspath v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/vfs v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/vfs/osvfs v0.0.0
	github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest v0.0.0
)

require (
	github.com/Microsoft/go-winio v0.6.2 // indirect
	github.com/dlclark/regexp2 v1.11.5 // indirect
	github.com/go-json-experiment/json v0.0.0-20260214004413-d219187c3433 // indirect
	github.com/google/go-cmp v0.7.0 // indirect
	github.com/klauspost/cpuid/v2 v2.2.10 // indirect
	github.com/mackerelio/go-osstat v0.2.7 // indirect
	github.com/microsoft/TypeScript/tsc/shim/collections v0.0.0 // indirect
	github.com/peter-evans/patience v0.3.0 // indirect
	github.com/zeebo/xxh3 v1.1.0 // indirect
	golang.org/x/sync v0.19.0 // indirect
	golang.org/x/sys v0.41.0 // indirect
	golang.org/x/text v0.34.0 // indirect
	gotest.tools/v3 v3.5.2 // indirect
)

replace github.com/effect-ts/tsgo/etscore => ./etscore

ignore (
	./.repos
	./.specs
	./_packages
	./_tools
	./node_modules
	./testdata/tests
)
