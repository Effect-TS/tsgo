package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// ScopedImmediateScopeToScopedWith suggests Effect.scopedWith when
// Effect.scoped wraps a body that immediately reads the provided scope back
// with Effect.scope.
var ScopedImmediateScopeToScopedWith = rule.Rule{
	Name:            "scopedImmediateScopeToScopedWith",
	Group:           "style",
	Description:     "Suggests Effect.scopedWith instead of Effect.scoped around a body that immediately reads Effect.scope",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_scoped_provides_a_scope_that_this_body_immediately_reads_back_with_Effect_scope_Effect_scopedWith_passes_the_scope_to_the_callback_directly_effect_scopedImmediateScopeToScopedWith.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeScopedImmediateScopeToScopedWith(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_scoped_provides_a_scope_that_this_body_immediately_reads_back_with_Effect_scope_Effect_scopedWith_passes_the_scope_to_the_callback_directly_effect_scopedImmediateScopeToScopedWith,
				nil,
			)
		}
		return diags
	},
}

// ScopedImmediateScopeToScopedWithMatch holds the nodes needed by the
// diagnostic and quick fix.
type ScopedImmediateScopeToScopedWithMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Flow       *typeparser.PipingFlow
	// PrefixLength counts the transformations up to and including Effect.scoped;
	// the fix replaces them with the Effect.scopedWith call.
	PrefixLength int
	Scoped       typeparser.PipingFlowTransformation
	// Generator form: the Effect.gen callee as written, its generator, and the
	// name bound by the leading `const <name> = yield* Effect.scope`.
	GenCallee   *ast.Node
	GenFunction *ast.FunctionExpression
	BindingName string
	// Combinator form: the flatMap (or v3 scopeWith) callback.
	Callback *ast.Node
	// Fixable is false when the callback parameter would capture a name the
	// replacement evaluates outside the original generator body.
	Fixable bool
}

// AnalyzeScopedImmediateScopeToScopedWith finds piping flows that apply
// Effect.scoped to an Effect.gen whose first statement reads Effect.scope, or
// to Effect.flatMap(Effect.scope, f) (Effect.scopeWith(f) on Effect v3), when
// nothing else in the body reads or requires the ambient Scope.
func AnalyzeScopedImmediateScopeToScopedWith(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []ScopedImmediateScopeToScopedWithMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	noTypeArguments := func(transformation typeparser.PipingFlowTransformation) bool {
		return transformation.TypeArguments() == nil || len(transformation.TypeArguments().Nodes) == 0
	}
	isEffectApi := func(name string, argCount int, kinds ...typeparser.TransformationKind) func(typeparser.PipingFlowTransformation) bool {
		return func(transformation typeparser.PipingFlowTransformation) bool {
			if transformation.Callee() == nil || transformation.Args().Len() != argCount || !noTypeArguments(transformation) {
				return false
			}
			if len(kinds) > 0 && !slices.Contains(kinds, transformation.Kind()) {
				return false
			}
			for _, arg := range transformation.Args().All() {
				if arg == nil || arg.Kind == ast.KindSpreadElement {
					return false
				}
			}
			return tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), name)
		}
	}
	// Effect.fn post-transformations apply to a function body, not an expression.
	isScoped := isEffectApi("scoped", 0,
		typeparser.TransformationKindCall, typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable)
	isGen := isEffectApi("gen", 0, typeparser.TransformationKindCall)
	isFlatMap := isEffectApi("flatMap", 1,
		typeparser.TransformationKindCall, typeparser.TransformationKindDataFirst,
		typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable)
	isScopeWith := isEffectApi("scopeWith", 0)
	isGeneratorSubject := func(subject typeparser.PipingFlowSubject) bool {
		node := subject.Node()
		return node != nil && node.Kind == ast.KindFunctionExpression && node.AsFunctionExpression().AsteriskToken != nil
	}
	isEffectScope := func(node *ast.Node) bool {
		return node != nil && (ast.IsIdentifier(node) || ast.IsPropertyAccessExpression(node)) &&
			tp.IsNodeReferenceToEffectModuleApi(node, "scope")
	}
	isScopeSubject := func(subject typeparser.PipingFlowSubject) bool {
		return isEffectScope(subject.Node())
	}

	var matches []ScopedImmediateScopeToScopedWithMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		var match ScopedImmediateScopeToScopedWithMatch
		var ok bool
		switch {
		case flow.MatchesPrefix(isGeneratorSubject, isGen, isScoped):
			match, ok = analyzeScopedImmediateScopeGen(tp, c, flow, isEffectScope)
		case flow.MatchesPrefix(isScopeSubject, isFlatMap, isScoped):
			match, ok = analyzeScopedImmediateScopeCallback(tp, c, flow, flow.Transformations().At(0).Args().At(0), 2, isEffectScope)
		default:
			for _, sequence := range flow.FindTransformationSequences(isScopeWith, isScoped).All() {
				if callback := flow.TransformationInputNode(sequence.Start()); callback != nil {
					if match, ok = analyzeScopedImmediateScopeCallback(tp, c, flow, callback, sequence.Start()+2, isEffectScope); ok {
						break
					}
				}
			}
		}
		if !ok {
			continue
		}
		match.SourceFile = sf
		match.Flow = flow
		match.Scoped = flow.Transformations().At(match.PrefixLength - 1)
		match.Location = scanner.GetErrorRangeForNode(sf, match.Scoped.Callee())
		matches = append(matches, match)
	}
	return matches
}

func analyzeScopedImmediateScopeGen(tp *typeparser.TypeParser, c *checker.Checker, flow *typeparser.PipingFlow, isEffectScope func(*ast.Node) bool) (ScopedImmediateScopeToScopedWithMatch, bool) {
	genStep := flow.Transformations().At(0)
	gen := tp.EffectGenCall(genStep.Callee().Parent)
	// Options, pipe arguments and a return annotation pin the result
	// independently of the body.
	if gen == nil || gen.OptionsNode() != nil || gen.PipeArguments().Len() != 0 ||
		gen.GeneratorFunction().AsNode() != flow.Subject().Node() {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}
	genFn := gen.GeneratorFunction()
	if genFn.Type != nil || genFn.Modifiers() != nil || genFn.TypeParameters != nil && len(genFn.TypeParameters.Nodes) > 0 ||
		genFn.Body == nil || !ast.IsBlock(genFn.Body) {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}
	statements := genFn.Body.AsBlock().Statements
	if statements == nil || len(statements.Nodes) == 0 {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}

	// The first statement must be exactly `const <name> = yield* Effect.scope`.
	first := statements.Nodes[0]
	if !ast.IsVariableStatement(first) {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}
	declarationList := first.AsVariableStatement().DeclarationList
	if declarationList == nil || declarationList.Flags&ast.NodeFlagsBlockScoped != ast.NodeFlagsConst {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}
	declarations := declarationList.AsVariableDeclarationList().Declarations
	if declarations == nil || len(declarations.Nodes) != 1 {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}
	declaration := declarations.Nodes[0].AsVariableDeclaration()
	if declaration.Name() == nil || !ast.IsIdentifier(declaration.Name()) || declaration.Type != nil ||
		declaration.ExclamationToken != nil || declaration.Initializer == nil ||
		!ast.IsYieldExpression(declaration.Initializer) {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}
	retrieval := declaration.Initializer.AsYieldExpression()
	if retrieval.AsteriskToken == nil || retrieval.Expression == nil {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}
	operand := ast.SkipParentheses(retrieval.Expression)
	if !isEffectScope(operand) {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}

	// The retrieval must be the only Effect.scope reference in the body.
	if hasEffectScopeReference(genFn.Body, operand, isEffectScope) {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}

	// Every other yielded effect must keep its requirements under
	// Effect.scopedWith, which does not provide the ambient Scope.
	keepsRequirements := scopedWithKeepsRequirements(tp, c, flow, 2)
	retrievalNode := declaration.Initializer
	changesRequirements := checker.ForEachYieldExpression(genFn.Body, func(expr *ast.Node) bool {
		if expr == retrievalNode || expr.Kind != ast.KindYieldExpression || tp.GetEffectYieldGeneratorFunction(expr) != genFn {
			return false
		}
		yield := expr.AsYieldExpression()
		if yield.AsteriskToken == nil || yield.Expression == nil {
			return false
		}
		effect := tp.EffectYieldableTypeAtLocation(yield.Expression)
		return effect == nil || !keepsRequirements(effect.R())
	})
	if changesRequirements {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}

	bindingName := declaration.Name().Text()
	return ScopedImmediateScopeToScopedWithMatch{
		PrefixLength: 2,
		GenCallee:    genStep.Callee(),
		GenFunction:  genFn,
		BindingName:  bindingName,
		// The new callback parameter scopes over the gen callee and the
		// generator's name and parameters, which the body binding did not.
		Fixable: !referencesName(genStep.Callee(), bindingName) && !referencesName(genFn.Name(), bindingName) &&
			(genFn.Parameters == nil || !slices.ContainsFunc(genFn.Parameters.Nodes, func(parameter *ast.Node) bool { return referencesName(parameter, bindingName) })),
	}, true
}

func analyzeScopedImmediateScopeCallback(tp *typeparser.TypeParser, c *checker.Checker, flow *typeparser.PipingFlow, callback *ast.Node, prefixLength int, isEffectScope func(*ast.Node) bool) (ScopedImmediateScopeToScopedWithMatch, bool) {
	if callback == nil || hasEffectScopeReference(callback, nil, isEffectScope) {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}
	// The flatMap and scopeWith results already require Scope, so the effect
	// the callback returns must keep its requirements under Effect.scopedWith.
	keepsRequirements := scopedWithKeepsRequirements(tp, c, flow, prefixLength)
	callbackType := tp.GetTypeAtLocation(callback)
	if callbackType == nil {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}
	callSignatures := c.GetSignaturesOfType(callbackType, checker.SignatureKindCall)
	if len(callSignatures) == 0 {
		return ScopedImmediateScopeToScopedWithMatch{}, false
	}
	for _, signature := range callSignatures {
		effect := tp.EffectType(c.GetReturnTypeOfSignature(signature))
		if effect == nil || !keepsRequirements(effect.R()) {
			return ScopedImmediateScopeToScopedWithMatch{}, false
		}
	}
	return ScopedImmediateScopeToScopedWithMatch{
		PrefixLength: prefixLength,
		Callback:     callback,
		Fixable:      true,
	}, true
}

// hasEffectScopeReference reports whether node contains a reference to
// Effect.scope other than allowed, skipping type positions.
func hasEffectScopeReference(node *ast.Node, allowed *ast.Node, isEffectScope func(*ast.Node) bool) bool {
	found := false
	var visit func(*ast.Node) bool
	visit = func(current *ast.Node) bool {
		if found || current == nil || ast.IsTypeNode(current) {
			return found
		}
		switch {
		case ast.IsPropertyAccessExpression(current):
			if current != allowed && isEffectScope(current) {
				found = true
				return true
			}
			// The member name is part of this access, not a reference of its own.
			return visit(current.AsPropertyAccessExpression().Expression)
		case ast.IsIdentifier(current):
			if current != allowed && isEffectScope(current) {
				found = true
			}
			return found
		}
		return current.ForEachChild(visit)
	}
	visit(node)
	return found
}

// scopedWithKeepsRequirements returns a check that an effect moved from
// Effect.scoped to Effect.scopedWith, which does not provide the ambient
// Scope, neither needs that Scope nor changes the result type.
//
// Each union member of its requirements must be independent of Scope: not
// any, unknown or generic, and neither assignable to Scope nor a supertype of
// it, since any of those may stand for a Scope the effect reads at runtime.
// Comparing against the combined requirements alone cannot show that, because
// broad combined requirements such as unknown accept Scope too.
//
// The requirements must also be assignable to what Effect.scoped produced,
// Exclude<R, Scope>, so the rewrite, which yields them unchanged, keeps the
// result type.
func scopedWithKeepsRequirements(tp *typeparser.TypeParser, c *checker.Checker, flow *typeparser.PipingFlow, prefixLength int) func(*checker.Type) bool {
	scopedIndex := prefixLength - 1
	scoped := tp.EffectType(flow.Transformations().At(scopedIndex).OutType())
	// The retrieval puts Scope in the requirements Effect.scoped receives.
	var scopeType *checker.Type
	if input := tp.EffectType(flow.TransformationInputType(scopedIndex)); input != nil {
		for _, member := range tp.UnrollUnionMembers(input.R()) {
			if tp.ScopeType(member) != nil {
				scopeType = member
				break
			}
		}
	}
	independentOfScope := func(member *checker.Type) bool {
		if member.Flags()&checker.TypeFlagsNever != 0 {
			return true
		}
		return member.Flags()&(checker.TypeFlagsAnyOrUnknown|checker.TypeFlagsInstantiable) == 0 &&
			tp.ScopeType(member) == nil &&
			!checker.Checker_isTypeAssignableTo(c, member, scopeType) &&
			!checker.Checker_isTypeAssignableTo(c, scopeType, member)
	}
	return func(requirements *checker.Type) bool {
		if scoped == nil || scoped.R() == nil || scopeType == nil || requirements == nil {
			return false
		}
		for _, member := range tp.UnrollUnionMembers(requirements) {
			if !independentOfScope(member) {
				return false
			}
		}
		return checker.Checker_isTypeAssignableTo(c, requirements, scoped.R())
	}
}

// referencesName reports whether node contains an identifier named name,
// other than a property access member name.
func referencesName(node *ast.Node, name string) bool {
	var visit func(*ast.Node) bool
	visit = func(current *ast.Node) bool {
		if current == nil {
			return false
		}
		switch {
		case ast.IsPropertyAccessExpression(current):
			return visit(current.AsPropertyAccessExpression().Expression)
		case ast.IsIdentifier(current):
			return current.Text() == name
		}
		return current.ForEachChild(visit)
	}
	return visit(node)
}
