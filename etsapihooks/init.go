package etsapihooks

import (
	"context"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/effectconfigraw"
	"github.com/effect-ts/tsgo/internal/rulerunner"
	"github.com/microsoft/typescript-go/shim/api"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
)

func init() {
	effectconfigraw.Register()
	api.RegisterGetEffectDiagnosticsCallback(getEffectDiagnostics)
}

func getEffectDiagnostics(ctx context.Context, program *compiler.Program, c *checker.Checker, sf *ast.SourceFile, options *etscore.EffectPluginOptions, ruleNames []string) ([]*ast.Diagnostic, error) {
	if options == nil {
		options = program.Options().Effect
	}
	return rulerunner.Run(ctx, program, c, sf, options, ruleNames)
}
