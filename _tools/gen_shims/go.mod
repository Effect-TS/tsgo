module github.com/effect-ts/tsgo/_tools/gen_shims

go 1.26

require (
	golang.org/x/mod v0.37.0
	golang.org/x/text v0.38.0
	golang.org/x/tools v0.47.0
)

require (
	github.com/google/go-cmp v0.7.0 // indirect
	golang.org/x/sync v0.21.0 // indirect
)

ignore (
	./config
	./providers
)
