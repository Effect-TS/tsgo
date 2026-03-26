package rules

import (
	"fmt"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// OutdatedApi detects usage of Effect v3 APIs in projects targeting Effect v4.
var OutdatedApi = rule.Rule{
	Name:            "outdatedApi",
	Group:           "correctness",
	Description:     "Detects usage of APIs that have been removed or renamed in Effect v4",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.X_0_is_an_Effect_v3_API_but_the_project_is_using_Effect_v4_1_effect_outdatedApi.Code(),
		tsdiag.This_project_targets_Effect_v4_but_is_using_Effect_v3_APIs_To_find_the_correct_API_to_use_consult_the_Effect_v4_documentation_for_the_corresponding_v4_replacement_effect_outdatedApi.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeOutdatedApi(ctx.Checker, ctx.SourceFile)
		if len(matches) == 0 {
			return nil
		}

		diags := make([]*ast.Diagnostic, 0, len(matches)+1)
		for _, m := range matches {
			diags = append(diags, ctx.NewDiagnostic(
				m.SourceFile,
				m.Location,
				tsdiag.X_0_is_an_Effect_v3_API_but_the_project_is_using_Effect_v4_1_effect_outdatedApi,
				nil,
				m.PropertyName,
				m.MigrationHint,
			))
		}

		// Append global summary diagnostic at position 0:0
		diags = append(diags, ctx.NewDiagnostic(
			ctx.SourceFile,
			core.NewTextRange(0, 0),
			tsdiag.This_project_targets_Effect_v4_but_is_using_Effect_v3_APIs_To_find_the_correct_API_to_use_consult_the_Effect_v4_documentation_for_the_corresponding_v4_replacement_effect_outdatedApi,
			nil,
		))

		return diags
	},
}

// OutdatedApiMatch holds the analysis results for one outdated API usage.
type OutdatedApiMatch struct {
	SourceFile    *ast.SourceFile
	Location      core.TextRange
	PropertyName  string
	MigrationHint string
	Migration     Migration
}

// AnalyzeOutdatedApi finds all usages of Effect v3 APIs in an Effect v4 project.
func AnalyzeOutdatedApi(c *checker.Checker, sf *ast.SourceFile) []OutdatedApiMatch {
	if typeparser.SupportedEffectVersion(c) != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []OutdatedApiMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindPropertyAccessExpression {
			if m := checkOutdatedApiPropertyAccess(c, sf, n); m != nil {
				matches = append(matches, *m)
			}
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())

	return matches
}

func checkOutdatedApiPropertyAccess(c *checker.Checker, sf *ast.SourceFile, n *ast.Node) *OutdatedApiMatch {
	prop := n.AsPropertyAccessExpression()
	if prop == nil {
		return nil
	}

	nameNode := prop.Name()
	if nameNode == nil {
		return nil
	}

	identifierName := nameNode.Text()
	if identifierName == "" {
		return nil
	}

	migration, ok := EffectModuleMigrationDb[identifierName]
	if !ok {
		return nil
	}

	// Skip unchanged entries — they exist in both v3 and v4
	if migration.Tag == MigrationTagUnchanged {
		return nil
	}

	// Get the type of the expression (left side of the property access)
	expr := prop.Expression
	if expr == nil {
		return nil
	}

	targetType := typeparser.GetTypeAtLocation(c, expr)
	if targetType == nil {
		return nil
	}

	// Only report if the property does NOT exist on the target type
	// (confirming it's a v3-only API)
	if c.GetPropertyOfType(targetType, identifierName) != nil {
		return nil
	}

	// Verify the expression references the Effect module
	if !typeparser.IsExpressionEffectModule(c, expr) {
		return nil
	}

	return &OutdatedApiMatch{
		SourceFile:    sf,
		Location:      scanner.GetErrorRangeForNode(sf, nameNode),
		PropertyName:  identifierName,
		MigrationHint: migrationHintText(migration),
		Migration:     migration,
	}
}

func migrationHintText(m Migration) string {
	switch m.Tag {
	case MigrationTagRemoved:
		return m.AlternativePattern
	case MigrationTagRenamedSameBehaviour:
		return fmt.Sprintf("Renamed to %s.", m.NewName)
	case MigrationTagRenamedAndNeedsOptions:
		return fmt.Sprintf("Renamed to %s. %s", m.NewName, m.OptionsInstructions)
	default:
		return ""
	}
}
