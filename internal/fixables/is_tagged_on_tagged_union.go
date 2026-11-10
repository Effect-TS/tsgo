package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var IsTaggedOnTaggedUnionFix = fixable.Fixable{
	Name:        "isTaggedOnTaggedUnion",
	Description: "Replace Predicate.isTagged with Predicate.isTaggedStrict or the data type's guard",
	ErrorCodes: []int32{
		tsdiag.Predicate_isTagged_accepts_any_value_so_on_this_tagged_union_it_does_not_check_the_tag_against_the_members_and_narrows_less_than_Predicate_isTaggedStrict_effect_isTaggedOnTaggedUnion.Code(),
	},
	FixIDs: []string{"isTaggedOnTaggedUnion_fix"},
	Run:    runIsTaggedOnTaggedUnionFix,
}

func runIsTaggedOnTaggedUnionFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeIsTaggedOnTaggedUnion(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		// A tag outside the union has no replacement that keeps the meaning.
		if !match.KnownTag {
			continue
		}
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		transformation := match.Flow.Transformations().At(match.Index)
		fix := isTaggedGuardFix(ctx, match, transformation)
		if fix == nil {
			fix = isTaggedStrictFix(ctx, match, transformation)
		}
		if fix == nil {
			continue
		}
		if action := ctx.NewFixAction(*fix); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

// isTaggedGuardFix replaces isTagged with the is<Tag> guard of Effect's
// Option, Exit or Result, when the union is that data type and its module is
// reachable.
func isTaggedGuardFix(ctx *fixable.Context, match rules.IsTaggedOnTaggedUnionMatch, transformation typeparser.PipingFlowTransformation) *fixable.FixAction {
	if match.Module == "" {
		return nil
	}
	member := taggedDataTypeGuard(ctx, match.Module, "is"+match.Tag)
	guard := resolveModuleMember(ctx, transformation.Callee(), typeparser.FindModuleReference(match.SourceFile, match.Module), member)
	if guard == nil {
		return nil
	}
	return &fixable.FixAction{Description: "Replace with " + match.Module + "." + member.name, Run: func(tracker *rewriter.Tracker) {
		if match.Applied {
			tracker.ReplacePipingFlowTransformation(match.SourceFile, &transformation,
				rewriter.PipingFlowTransformationReplacement{Callee: guard.expression(tracker, member)})
			return
		}
		// An unapplied isTagged(tag) takes the tag as its input, so the guard
		// replaces the call and its input.
		tracker.ReplacePipingFlowPrefix(match.SourceFile, match.Flow, match.Index+1, guard.expression(tracker, member))
	}}
}

// isTaggedStrictFix replaces isTagged with Predicate.isTaggedStrict, written
// with the qualifier the callee has, keeping the arguments. It is offered only
// when the installed Effect exports isTaggedStrict.
func isTaggedStrictFix(ctx *fixable.Context, match rules.IsTaggedOnTaggedUnionMatch, transformation typeparser.PipingFlowTransformation) *fixable.FixAction {
	member := moduleMember{
		name:     "isTaggedStrict",
		isModule: ctx.TypeParser.IsExpressionEffectPredicateModule,
		isMember: func(node *ast.Node) bool {
			return ctx.TypeParser.IsNodeReferenceToEffectPredicateModuleApi(node, "isTaggedStrict")
		},
	}
	strict, namedImports := calleeModuleMemberReference(ctx, transformation.Callee(), typeparser.FindModuleReference(match.SourceFile, "Predicate"), member)
	if strict == nil {
		return nil
	}
	return &fixable.FixAction{Description: "Replace with Predicate.isTaggedStrict", Run: func(tracker *rewriter.Tracker) {
		var arguments *ast.NodeList
		if transformation.Args().Len() > 0 {
			nodes := make([]*ast.Node, 0, transformation.Args().Len())
			for _, argument := range transformation.Args().All() {
				nodes = append(nodes, tracker.DeepCloneNode(argument))
			}
			arguments = tracker.NewNodeList(nodes)
		}
		replaced := tracker.ReplacePipingFlowTransformation(match.SourceFile, &transformation,
			rewriter.PipingFlowTransformationReplacement{Callee: strict.expression(tracker, member), Arguments: arguments})
		if replaced && namedImports != nil {
			elements := namedImports.AsNamedImports().Elements
			tracker.InsertImportSpecifierAtIndex(match.SourceFile,
				tracker.NewImportSpecifier(false, nil, tracker.NewIdentifier(member.name)), namedImports, len(elements.Nodes))
		}
	}}
}

// taggedDataTypeGuard is the is<Tag> guard exported by one of the modules
// TypeParser.TaggedDataTypeModule returns.
func taggedDataTypeGuard(ctx *fixable.Context, module string, name string) moduleMember {
	tp := ctx.TypeParser
	switch module {
	case "Option":
		return moduleMember{name: name, isModule: tp.IsExpressionEffectOptionModule,
			isMember: func(node *ast.Node) bool { return tp.IsNodeReferenceToEffectOptionModuleApi(node, name) }}
	case "Exit":
		return moduleMember{name: name, isModule: tp.IsExpressionEffectExitModule,
			isMember: func(node *ast.Node) bool { return tp.IsNodeReferenceToEffectExitModuleApi(node, name) }}
	default:
		return moduleMember{name: name, isModule: tp.IsExpressionEffectResultModule,
			isMember: func(node *ast.Node) bool { return tp.IsNodeReferenceToEffectResultModuleApi(node, name) }}
	}
}
