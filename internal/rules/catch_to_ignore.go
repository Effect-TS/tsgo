// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// CatchToIgnore suggests using Effect.ignore or Effect.ignoreCause instead of Effect.catch/catchCause + Effect.void.
var CatchToIgnore = rule.Rule{
	Name:            "catchToIgnore",
	Group:           "style",
	Description:     "Suggests using Effect.ignore or Effect.ignoreCause instead of Effect.catch/catchCause returning Effect.void",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.Effect_1_expresses_ignored_failure_more_directly_than_Effect_0_returning_Effect_void_effect_catchToIgnore.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeCatchToIgnore(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Effect_1_expresses_ignored_failure_more_directly_than_Effect_0_returning_Effect_void_effect_catchToIgnore, nil, m.CatchMethodName, m.IgnoreMethodName)
		}
		return diags
	},
}

// CatchToIgnoreMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the catchToIgnore pattern.
type CatchToIgnoreMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange
	Transformation   *typeparser.PipingFlowTransformation
	EffectModuleNode *ast.Node
	CatchMethodName  string
	IgnoreMethodName string
}

// AnalyzeCatchToIgnore finds Effect.catch/catchCause callbacks that return Effect.void
// where the resulting success channel is void-like, so Effect.ignore/ignoreCause is equivalent.
func AnalyzeCatchToIgnore(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []CatchToIgnoreMatch {
	if tp == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []CatchToIgnoreMatch

	flows := tp.PipingFlows(sf, true)
	for _, flow := range flows {
		for index := range flow.Transformations {
			transformation := &flow.Transformations[index]
			catchMethodName, ignoreMethodName, effectModuleNode, ok := catchToIgnoreMethods(tp, transformation.Callee)
			if !ok {
				continue
			}

			if len(transformation.Args) < 1 {
				continue
			}

			lazy := typeparser.ParseLazyExpression(transformation.Args[0], typeparser.LazyExpressionNone)
			if lazy == nil || !isEffectVoidReference(tp, lazy.Expression) {
				continue
			}

			effect := tp.StrictEffectType(transformation.OutType)
			if effect == nil || !isVoidLikeEffectSuccess(effect.A) {
				continue
			}

			matches = append(matches, CatchToIgnoreMatch{
				SourceFile:       sf,
				Location:         scanner.GetErrorRangeForNode(sf, transformation.Callee),
				Transformation:   transformation,
				EffectModuleNode: effectModuleNode,
				CatchMethodName:  catchMethodName,
				IgnoreMethodName: ignoreMethodName,
			})
		}
	}

	return matches
}

func catchToIgnoreMethods(tp *typeparser.TypeParser, callee *ast.Node) (catchMethodName string, ignoreMethodName string, effectModuleNode *ast.Node, ok bool) {
	if callee == nil || callee.Kind != ast.KindPropertyAccessExpression {
		return "", "", nil, false
	}
	prop := callee.AsPropertyAccessExpression()
	if prop == nil || prop.Name() == nil {
		return "", "", nil, false
	}

	name := prop.Name().Text()
	switch name {
	case "catch":
		if !tp.IsNodeReferenceToEffectModuleApi(callee, "catch") {
			return "", "", nil, false
		}
		return name, "ignore", prop.Expression, true
	case "catchCause":
		if !tp.IsNodeReferenceToEffectModuleApi(callee, "catchCause") {
			return "", "", nil, false
		}
		return name, "ignoreCause", prop.Expression, true
	default:
		return "", "", nil, false
	}
}

func isEffectVoidReference(tp *typeparser.TypeParser, node *ast.Node) bool {
	if tp == nil || node == nil {
		return false
	}
	return tp.IsNodeReferenceToEffectModuleApi(ast.SkipParentheses(node), "void")
}

func isVoidLikeEffectSuccess(t *checker.Type) bool {
	if t == nil {
		return false
	}
	flags := t.Flags()
	if flags&(checker.TypeFlagsVoidLike|checker.TypeFlagsNever) != 0 {
		return true
	}
	if flags&checker.TypeFlagsUnion == 0 {
		return false
	}
	types := t.Types()
	if len(types) == 0 {
		return false
	}
	for _, member := range types {
		if !isVoidLikeEffectSuccess(member) {
			return false
		}
	}
	return true
}
