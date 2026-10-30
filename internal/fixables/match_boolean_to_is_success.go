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

var MatchBooleanToIsSuccessFix = fixable.Fixable{
	Name:        "matchBooleanToIsSuccess",
	Description: "Replace Effect.match boolean folds with Effect.isSuccess or Effect.isFailure",
	ErrorCodes: []int32{
		tsdiag.Effect_isSuccess_expresses_this_check_more_directly_than_Effect_0_folding_to_complementary_boolean_literals_effect_matchBooleanToIsSuccess.Code(),
		tsdiag.Effect_isFailure_expresses_this_check_more_directly_than_Effect_0_folding_to_complementary_boolean_literals_effect_matchBooleanToIsSuccess.Code(),
	},
	FixIDs: []string{"matchBooleanToIsSuccess_fix"},
	Run:    runMatchBooleanToIsSuccessFix,
}

func runMatchBooleanToIsSuccessFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeMatchBooleanToIsSuccess(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.Flow == nil || match.Callee == nil {
			continue
		}
		member := effectModuleMember(ctx, match.ReplacementName)
		reference, namedImports := matchBooleanReplacementReference(ctx, match.Callee, member)
		if reference == nil {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(tracker *rewriter.Tracker) {
				replacement := reference.expression(tracker, member)
				if match.PointFree {
					// match({...}) becomes the bare isSuccess / isFailure function.
					tracker.ReplacePipingFlowPrefix(match.SourceFile, match.Flow, 1, replacement)
				} else {
					transformation := match.Flow.Transformations().At(match.Index)
					tracker.ReplacePipingFlowTransformation(match.SourceFile, &transformation, rewriter.PipingFlowTransformationReplacement{Callee: replacement})
				}
				if namedImports != nil {
					elements := namedImports.AsNamedImports().Elements
					tracker.InsertImportSpecifierAtIndex(match.SourceFile,
						tracker.NewImportSpecifier(false, nil, tracker.NewIdentifier(member.name)), namedImports, len(elements.Nodes))
				}
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

// matchBooleanReplacementReference keeps the qualifier the match callee is
// written with: Q.match gives Q.isSuccess, a const alias of Q.match gives
// Q.isSuccess, and a named import of match gives a bare isSuccess from the same
// import. When the bare name has to be imported, namedImports is the list to
// add it to. Otherwise it falls back to the file's Effect module reference.
func matchBooleanReplacementReference(ctx *fixable.Context, callee *ast.Node, member moduleMember) (reference *moduleMemberReference, namedImports *ast.Node) {
	if qualifier := matchBooleanCalleeQualifier(ctx, callee); qualifier != nil {
		if reference := importedModuleMember(ctx, callee, qualifier, member); reference != nil {
			return reference, nil
		}
	}
	if reference, namedImports := namedImportModuleMember(ctx, callee, member); reference != nil {
		return reference, namedImports
	}
	return resolveModuleMember(ctx, callee, typeparser.FindEffectModuleReference(ctx.SourceFile), member), nil
}

// matchBooleanCalleeQualifier returns the Effect module path of a Q.match
// callee, or of the initializer of a const alias of Q.match.
func matchBooleanCalleeQualifier(ctx *fixable.Context, callee *ast.Node) typeparser.ModuleReference {
	callee = ast.SkipParentheses(callee)
	if ast.IsIdentifier(callee) {
		declaration := ast.TSGetSymbolValueDeclaration(ctx.TypeParser.GetSymbolAtLocation(callee))
		if declaration == nil || !ast.IsVariableDeclaration(declaration) || declaration.Parent == nil ||
			declaration.Parent.Flags&ast.NodeFlagsConst == 0 {
			return nil
		}
		callee = declaration.Initializer()
		if callee == nil {
			return nil
		}
		callee = ast.SkipParentheses(callee)
	}
	if !ast.IsPropertyAccessExpression(callee) {
		return nil
	}
	qualifier := callee.Expression()
	if !ctx.TypeParser.IsExpressionEffectModule(qualifier) {
		return nil
	}
	return identifierPath(qualifier)
}

// identifierPath returns the names of an identifier or a chain of property
// accesses on identifiers, or nil for any other expression.
func identifierPath(node *ast.Node) typeparser.ModuleReference {
	switch {
	case ast.IsIdentifier(node):
		return typeparser.ModuleReference{node.Text()}
	case ast.IsPropertyAccessExpression(node) && ast.IsIdentifier(node.Name()):
		if path := identifierPath(node.Expression()); path != nil {
			return path.Append(node.Name().Text())
		}
	}
	return nil
}

// namedImportModuleMember handles a callee that is a named import binding of
// the match API: it reuses a bare member binding already in scope, or returns
// the bare member name with the callee's NamedImports to add it to when the
// name is free and the imported module exports it.
func namedImportModuleMember(ctx *fixable.Context, callee *ast.Node, member moduleMember) (*moduleMemberReference, *ast.Node) {
	callee = ast.SkipParentheses(callee)
	if !ast.IsIdentifier(callee) {
		return nil, nil
	}
	binding := importBindingName(ctx, callee, ctx.Checker.ResolveName(callee.Text(), callee, ast.SymbolFlagsValue|ast.SymbolFlagsAlias, false))
	if binding == nil || binding.Parent == nil || !ast.IsImportSpecifier(binding.Parent) {
		return nil, nil
	}
	namedImports := binding.Parent.Parent
	if namedImports == nil || !ast.IsNamedImports(namedImports) || namedImports.Parent == nil || namedImports.Parent.Parent == nil ||
		!ast.IsImportDeclaration(namedImports.Parent.Parent) {
		return nil, nil
	}
	moduleSymbol := ctx.TypeParser.GetSymbolAtLocation(namedImports.Parent.Parent.AsImportDeclaration().ModuleSpecifier)
	if moduleSymbol == nil {
		return nil, nil
	}
	export := ctx.Checker.TryGetMemberInModuleExports(member.name, moduleSymbol)
	if export == nil {
		return nil, nil
	}
	if ast.TSGetSymbolFlags(export)&ast.SymbolFlagsAlias != 0 {
		export = ctx.Checker.GetAliasedSymbol(export)
	}
	exportDeclaration := ast.TSGetSymbolValueDeclaration(export)
	if exportDeclaration == nil || exportDeclaration.Name() == nil || !member.isMember(exportDeclaration.Name()) {
		return nil, nil
	}

	existing := ctx.Checker.ResolveName(member.name, callee, ast.SymbolFlagsValue|ast.SymbolFlagsAlias, false)
	if existing == nil {
		return &moduleMemberReference{path: typeparser.ModuleReference{member.name}}, namedImports
	}
	if name := importBindingName(ctx, callee, existing); name != nil && member.isMember(name) {
		return &moduleMemberReference{path: typeparser.ModuleReference{name.Text()}}, nil
	}
	return nil, nil
}
