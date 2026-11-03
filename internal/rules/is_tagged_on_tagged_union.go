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

// IsTaggedOnTaggedUnion reports Predicate.isTagged on a value whose type is
// already a union of tagged members. isTagged accepts unknown, so a misspelled
// tag compiles, a member with no field but its _tag survives a failed test, and
// a refinement passed as a callback replaces the element type with { _tag: K }.
// A `_tag` comparison has none of these problems.
var IsTaggedOnTaggedUnion = rule.Rule{
	Name:            "isTaggedOnTaggedUnion",
	Group:           "antipattern",
	Description:     "Reports Predicate.isTagged on a value already typed as a tagged union, where a _tag comparison checks the tag and narrows",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Predicate_isTagged_accepts_any_value_so_on_this_tagged_union_it_neither_checks_the_tag_against_the_members_nor_narrows_as_a_tag_comparison_does_effect_isTaggedOnTaggedUnion.Code(),
		tsdiag.X_0_is_not_a_tag_of_this_tagged_union_so_this_Predicate_isTagged_test_is_always_false_effect_isTaggedOnTaggedUnion.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeIsTaggedOnTaggedUnion(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			if match.KnownTag {
				diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.Predicate_isTagged_accepts_any_value_so_on_this_tagged_union_it_neither_checks_the_tag_against_the_members_nor_narrows_as_a_tag_comparison_does_effect_isTaggedOnTaggedUnion, nil)
			} else {
				diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.X_0_is_not_a_tag_of_this_tagged_union_so_this_Predicate_isTagged_test_is_always_false_effect_isTaggedOnTaggedUnion, nil, match.Tag)
			}
		}
		return diagnostics
	},
}

// IsTaggedOnTaggedUnionMatch holds the nodes needed by the diagnostic and quick fix.
type IsTaggedOnTaggedUnionMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	// Flow contains the isTagged step at Index.
	Flow  *typeparser.PipingFlow
	Index int
	Tag   string
	// KnownTag is false when Tag names no member of the union.
	KnownTag bool
	// Applied is true for isTagged(x, tag), isTagged(tag)(x) and
	// pipe(x, isTagged(tag)), false for isTagged(tag) passed as a callback.
	Applied bool
	// Module is the Effect module that declares the union's members (see
	// TypeParser.TaggedDataTypeModule), or "".
	Module string
}

// AnalyzeIsTaggedOnTaggedUnion finds Predicate.isTagged steps whose tested
// value is typed as a union of members with string literal tags.
func AnalyzeIsTaggedOnTaggedUnion(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []IsTaggedOnTaggedUnionMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []IsTaggedOnTaggedUnionMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for index := range flow.Transformations().All() {
			transformation := flow.Transformations().At(index)
			if transformation.Args().Len() > 1 || !tp.IsNodeReferenceToEffectPredicateModuleApi(transformation.Callee(), "isTagged") {
				continue
			}
			applied := transformation.Args().Len() == 1
			var tagType, valueType *checker.Type
			if applied {
				tagType = tp.GetTypeAtLocation(transformation.Args().At(0))
				valueType = flow.TransformationInputType(index)
			} else {
				// isTagged(tag) alone takes the tag as its input and returns a
				// refinement, which tests the parameter of the callback it is passed as.
				tagType = flow.TransformationInputType(index)
				valueType = isTaggedCallbackParameterType(tp, c, transformation.Callee())
			}
			if tagType == nil || tagType.Flags()&checker.TypeFlagsStringLiteral == 0 {
				continue
			}
			tag, ok := tagType.AsLiteralType().Value().(string)
			if !ok {
				continue
			}
			tags, ok := literalTaggedUnionTypeTags(tp, c, valueType)
			if !ok {
				continue
			}
			_, known := tags[tag]
			matches = append(matches, IsTaggedOnTaggedUnionMatch{
				SourceFile: sf, Location: scanner.GetErrorRangeForNode(sf, transformation.Callee()),
				Flow: flow, Index: index,
				Tag: tag, KnownTag: known,
				Applied: applied,
				Module:  tp.TaggedDataTypeModule(valueType),
			})
		}
	}
	return matches
}

// isTaggedCallbackParameterType returns the first parameter type of the
// callback signature an unapplied isTagged(tag) call is contextually typed by.
func isTaggedCallbackParameterType(tp *typeparser.TypeParser, c *checker.Checker, callee *ast.Node) *checker.Type {
	call := callee.Parent
	if call == nil || !ast.IsCallExpression(call) || call.AsCallExpression().Expression != callee {
		return nil
	}
	// isTagged's own unknown parameter takes part in inferring the call it is
	// passed to: Effect's data-first Array.filter infers unknown elements from
	// it. The contextual type without the argument's inferences comes first.
	contextualType := tp.CachedExpressionConstraintContextualType(call)
	if contextualType == nil {
		contextualType = tp.GetContextualType(call, checker.ContextFlagsNone)
	}
	if contextualType == nil {
		return nil
	}
	signatures := c.GetSignaturesOfType(contextualType, checker.SignatureKindCall)
	if len(signatures) != 1 || len(signatures[0].Parameters()) == 0 {
		return nil
	}
	return c.GetTypeAtPosition(signatures[0], 0)
}
