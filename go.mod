module github.com/effect-ts/effect-typescript-go

go 1.26

replace (
	github.com/microsoft/typescript-go => ./typescript-go
	github.com/microsoft/typescript-go/shim/ast => ./shim/ast
	github.com/microsoft/typescript-go/shim/astnav => ./shim/astnav
	github.com/microsoft/typescript-go/shim/bundled => ./shim/bundled
	github.com/microsoft/typescript-go/shim/checker => ./shim/checker
	github.com/microsoft/typescript-go/shim/collections => ./shim/collections
	github.com/microsoft/typescript-go/shim/compiler => ./shim/compiler
	github.com/microsoft/typescript-go/shim/core => ./shim/core
	github.com/microsoft/typescript-go/shim/diagnostics => ./shim/diagnostics
	github.com/microsoft/typescript-go/shim/execute/tsc => ./shim/execute/tsc
	github.com/microsoft/typescript-go/shim/format => ./shim/format
	github.com/microsoft/typescript-go/shim/fourslash => ./shim/fourslash
	github.com/microsoft/typescript-go/shim/ls => ./shim/ls
	github.com/microsoft/typescript-go/shim/ls/autoimport => ./shim/ls/autoimport
	github.com/microsoft/typescript-go/shim/ls/change => ./shim/ls/change
	github.com/microsoft/typescript-go/shim/ls/lsconv => ./shim/ls/lsconv
	github.com/microsoft/typescript-go/shim/ls/lsutil => ./shim/ls/lsutil
	github.com/microsoft/typescript-go/shim/lsp => ./shim/lsp
	github.com/microsoft/typescript-go/shim/lsp/lsproto => ./shim/lsp/lsproto
	github.com/microsoft/typescript-go/shim/modulespecifiers => ./shim/modulespecifiers
	github.com/microsoft/typescript-go/shim/packagejson => ./shim/packagejson
	github.com/microsoft/typescript-go/shim/parser => ./shim/parser
	github.com/microsoft/typescript-go/shim/project => ./shim/project
	github.com/microsoft/typescript-go/shim/repo => ./shim/repo
	github.com/microsoft/typescript-go/shim/scanner => ./shim/scanner
	github.com/microsoft/typescript-go/shim/testrunner => ./shim/testrunner
	github.com/microsoft/typescript-go/shim/testutil => ./shim/testutil
	github.com/microsoft/typescript-go/shim/testutil/baseline => ./shim/testutil/baseline
	github.com/microsoft/typescript-go/shim/testutil/harnessutil => ./shim/testutil/harnessutil
	github.com/microsoft/typescript-go/shim/testutil/tsbaseline => ./shim/testutil/tsbaseline
	github.com/microsoft/typescript-go/shim/tsoptions => ./shim/tsoptions
	github.com/microsoft/typescript-go/shim/tspath => ./shim/tspath
	github.com/microsoft/typescript-go/shim/vfs => ./shim/vfs
	github.com/microsoft/typescript-go/shim/vfs/cachedvfs => ./shim/vfs/cachedvfs
	github.com/microsoft/typescript-go/shim/vfs/iovfs => ./shim/vfs/iovfs
	github.com/microsoft/typescript-go/shim/vfs/osvfs => ./shim/vfs/osvfs
	github.com/microsoft/typescript-go/shim/vfs/vfstest => ./shim/vfs/vfstest
)

require (
	github.com/effect-ts/effect-typescript-go/etscore v0.0.0
	github.com/microsoft/typescript-go/shim/ast v0.0.0
	github.com/microsoft/typescript-go/shim/astnav v0.0.0
	github.com/microsoft/typescript-go/shim/bundled v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/checker v0.0.0
	github.com/microsoft/typescript-go/shim/compiler v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/core v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/diagnostics v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/execute/tsc v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/fourslash v0.0.0
	github.com/microsoft/typescript-go/shim/ls v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/ls/autoimport v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/ls/change v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/ls/lsconv v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/ls/lsutil v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/lsp/lsproto v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/modulespecifiers v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/packagejson v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/parser v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/scanner v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/testutil/baseline v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/testutil/harnessutil v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/tsoptions v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/tspath v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/vfs v0.0.0-00010101000000-000000000000
	github.com/microsoft/typescript-go/shim/vfs/vfstest v0.0.0-00010101000000-000000000000
)

require (
	github.com/Microsoft/go-winio v0.6.2 // indirect
	github.com/dlclark/regexp2 v1.11.5 // indirect
	github.com/go-json-experiment/json v0.0.0-20260214004413-d219187c3433 // indirect
	github.com/google/go-cmp v0.7.0 // indirect
	github.com/klauspost/cpuid/v2 v2.2.10 // indirect
	github.com/microsoft/typescript-go v0.0.0 // indirect
	github.com/microsoft/typescript-go/shim/collections v0.0.0 // indirect
	github.com/peter-evans/patience v0.3.0 // indirect
	github.com/zeebo/xxh3 v1.1.0 // indirect
	golang.org/x/sync v0.19.0 // indirect
	golang.org/x/sys v0.41.0 // indirect
	golang.org/x/text v0.34.0 // indirect
	gotest.tools/v3 v3.5.2 // indirect
)

replace github.com/effect-ts/effect-typescript-go/etscore => ./etscore

ignore (
	./.repos
	./.specs
	./_packages
	./_tools
	./node_modules
	./testdata/tests
)
