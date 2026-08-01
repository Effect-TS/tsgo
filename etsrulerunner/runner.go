// Package etsrulerunner exposes Effect diagnostic execution to external runners.
package etsrulerunner

import (
	"context"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rulerunner"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// RunRule executes one Effect rule with the supplied program checker.
func RunRule(
	ctx context.Context,
	program checker.Program,
	c *checker.Checker,
	sf *ast.SourceFile,
	options *etscore.EffectPluginOptions,
	ruleName string,
) ([]*ast.Diagnostic, error) {
	if options == nil {
		return nil, nil
	}

	normalized := *options
	normalized.Diagnostics = true
	normalized.DiagnosticSeverity = map[string]etscore.Severity{
		ruleName: etscore.SeverityError,
	}
	normalized.Overrides = append([]etscore.Override(nil), options.Overrides...)
	for i := range normalized.Overrides {
		normalized.Overrides[i].Options.DiagnosticSeverity = nil
	}

	return rulerunner.Run(ctx, program, c, sf, &normalized, []string{ruleName})
}
