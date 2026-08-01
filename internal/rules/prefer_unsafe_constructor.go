package rules

import (
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// PreferUnsafeConstructor detects Effect.runSync applied directly to an effect-package
// constructor call whose module also exports a `<name>Unsafe` sibling producing the same
// value synchronously (e.g. Effect.runSync(Scope.make()) -> Scope.makeUnsafe()).
var PreferUnsafeConstructor = rule.Rule{
	Name:            "preferUnsafeConstructor",
	Group:           "antipattern",
	Description:     "Suggests replacing Effect.runSync of a pure effect constructor with the synchronous *Unsafe variant exported by the same module",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.X_0_starts_a_fiber_to_run_a_pure_constructor_Use_the_synchronous_variant_1_instead_effect_preferUnsafeConstructor.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzePreferUnsafeConstructor(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, 0, len(matches))
		for _, m := range matches {
			originalText := scanner.GetSourceTextOfNodeFromSourceFile(m.SourceFile, m.OuterCallee, false) + "(" +
				scanner.GetSourceTextOfNodeFromSourceFile(m.SourceFile, m.InnerCallee, false) + "(...))"
			unsafeText := m.UnsafeCalleeText + "(...)"
			diags = append(diags, ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.X_0_starts_a_fiber_to_run_a_pure_constructor_Use_the_synchronous_variant_1_instead_effect_preferUnsafeConstructor, nil, originalText, unsafeText))
		}
		return diags
	},
}

// PreferUnsafeConstructorMatch holds the nodes needed by the diagnostic and quick fix.
type PreferUnsafeConstructorMatch struct {
	SourceFile       *ast.SourceFile // The source file where this match was found
	Location         core.TextRange  // Error range on the outer Effect.runSync call
	OuterCall        *ast.Node       // The Effect.runSync(...) call expression
	OuterCallee      *ast.Node       // The Effect.runSync callee expression
	InnerCall        *ast.Node       // The constructor call expression (e.g. Scope.make())
	InnerCallee      *ast.Node       // The constructor callee expression (e.g. Scope.make)
	InnerCalleeName  *ast.Node       // The identifier holding the constructor name (e.g. make)
	UnsafeName       string          // The sibling export name (e.g. makeUnsafe)
	UnsafeCalleeText string          // The callee text with the name replaced (e.g. Scope.makeUnsafe)
}

// AnalyzePreferUnsafeConstructor finds Effect.runSync calls whose single argument is a
// direct call to an effect-package constructor with a matching `<name>Unsafe` sibling export.
func AnalyzePreferUnsafeConstructor(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []PreferUnsafeConstructorMatch {
	var matches []PreferUnsafeConstructorMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindCallExpression {
			if m, ok := analyzePreferUnsafeConstructorNode(tp, c, sf, n); ok {
				matches = append(matches, m)
			}
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

// analyzePreferUnsafeConstructorNode checks a single call expression for the
// Effect.runSync(<effect constructor>()) pattern with a matching *Unsafe sibling.
func analyzePreferUnsafeConstructorNode(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, node *ast.Node) (PreferUnsafeConstructorMatch, bool) {
	call := node.AsCallExpression()
	if call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
		return PreferUnsafeConstructorMatch{}, false
	}
	if !tp.IsNodeReferenceToEffectModuleApi(call.Expression, "runSync") {
		return PreferUnsafeConstructorMatch{}, false
	}

	// The argument must itself be a direct constructor call, not a variable or composed effect.
	inner := ast.SkipParentheses(call.Arguments.Nodes[0])
	if inner == nil || inner.Kind != ast.KindCallExpression {
		return PreferUnsafeConstructorMatch{}, false
	}
	innerCall := inner.AsCallExpression()
	innerCallee := innerCall.Expression

	var nameNode *ast.Node
	switch innerCallee.Kind {
	case ast.KindIdentifier:
		nameNode = innerCallee
	case ast.KindPropertyAccessExpression:
		nameNode = innerCallee.AsPropertyAccessExpression().Name()
	default:
		return PreferUnsafeConstructorMatch{}, false
	}
	if nameNode == nil || nameNode.Kind != ast.KindIdentifier {
		return PreferUnsafeConstructorMatch{}, false
	}

	// Spread arguments make the argument list impossible to validate statically.
	if innerCall.Arguments != nil {
		for _, arg := range innerCall.Arguments.Nodes {
			if arg.Kind == ast.KindSpreadElement {
				return PreferUnsafeConstructorMatch{}, false
			}
		}
	}

	// The constructor call must produce an `Effect<A, never, never>`: a fallible or
	// service-requiring effect has no behavior-preserving synchronous replacement.
	eff := tp.EffectType(tp.GetTypeAtLocation(inner), inner)
	if eff == nil || eff.A == nil {
		return PreferUnsafeConstructorMatch{}, false
	}
	if eff.E == nil || eff.E.Flags()&checker.TypeFlagsNever == 0 {
		return PreferUnsafeConstructorMatch{}, false
	}
	if eff.R == nil || eff.R.Flags()&checker.TypeFlagsNever == 0 {
		return PreferUnsafeConstructorMatch{}, false
	}

	sym := tp.ReferenceSymbolAtNode(innerCallee)
	if sym == nil {
		return PreferUnsafeConstructorMatch{}, false
	}
	// Use the resolved export name: a named import may alias the local identifier
	// (e.g. `import { make as makeScope }`), and the sibling lives under the export name.
	name := sym.Name
	if name == "" || strings.HasSuffix(name, "Unsafe") {
		return PreferUnsafeConstructorMatch{}, false
	}

	unsafeName := name + "Unsafe"
	for _, decl := range sym.Declarations {
		if decl == nil {
			continue
		}
		declSf := ast.GetSourceFileOfNode(decl)
		if declSf == nil || !declSf.IsDeclarationFile || !tp.IsSourceFileInPackage(declSf, "effect") {
			continue
		}
		moduleSym := checker.Checker_getSymbolOfDeclaration(c, declSf.AsNode())
		if moduleSym == nil {
			continue
		}
		// The callee must be the module-level export of that name, not a nested member
		// (e.g. a method) that happens to be declared in an effect declaration file.
		exportSym := resolveAliasedSymbol(c, c.TryGetMemberInModuleExportsAndProperties(name, moduleSym))
		if checker.Checker_getSymbolIfSameReference(c, exportSym, sym) == nil {
			continue
		}
		sibling := resolveAliasedSymbol(c, c.TryGetMemberInModuleExportsAndProperties(unsafeName, moduleSym))
		if sibling == nil {
			continue
		}
		if !unsafeSiblingMatchesCall(tp, c, innerCall, sibling, eff.A) {
			continue
		}

		// For a property access like `Scope.make` the sibling stays reachable through the
		// same object, so `Scope.makeUnsafe` both renders in the message and drives the fix.
		// A bare identifier callee has no local binding for the sibling, so only the plain
		// export name is shown and no rename-based fix is possible.
		unsafeCalleeText := unsafeName
		if innerCallee.Kind == ast.KindPropertyAccessExpression {
			calleeText := scanner.GetSourceTextOfNodeFromSourceFile(sf, innerCallee, false)
			nameText := nameNode.Text()
			unsafeCalleeText = strings.TrimSuffix(calleeText, nameText) + unsafeName
		}
		return PreferUnsafeConstructorMatch{
			SourceFile:       sf,
			Location:         scanner.GetErrorRangeForNode(sf, node),
			OuterCall:        node,
			OuterCallee:      call.Expression,
			InnerCall:        inner,
			InnerCallee:      innerCallee,
			InnerCalleeName:  nameNode,
			UnsafeName:       unsafeName,
			UnsafeCalleeText: unsafeCalleeText,
		}, true
	}

	return PreferUnsafeConstructorMatch{}, false
}

// unsafeSiblingMatchesCall reports whether some call signature of the sibling symbol is
// applicable to the constructor call's exact argument list and produces a value usable
// where the runSync result flows, so the rewrite stays well typed.
func unsafeSiblingMatchesCall(tp *typeparser.TypeParser, c *checker.Checker, innerCall *ast.CallExpression, sibling *ast.Symbol, successType *checker.Type) bool {
	siblingType := c.GetTypeOfSymbol(sibling)
	if siblingType == nil {
		return false
	}

	resolved := c.GetResolvedSignature(innerCall.AsNode())
	if resolved == nil {
		return false
	}

	var args []*ast.Node
	if innerCall.Arguments != nil {
		args = innerCall.Arguments.Nodes
	}

	for _, sig := range c.GetSignaturesOfType(siblingType, checker.SignatureKindCall) {
		if sig == nil || !signatureAcceptsArguments(tp, c, sig, args) {
			continue
		}
		// Relate the applicable signature alone: calling the sibling where
		// `(constructor params) => <success type>` is expected. This instantiates
		// generic siblings against the concrete argument and result types.
		sigFnType := checker.Checker_newFunctionType(c, sig.TypeParameters(), sig.ThisParameter(), sig.Parameters(), c.GetReturnTypeOfSignature(sig))
		expected := checker.Checker_newFunctionType(c, nil, nil, resolved.Parameters(), successType)
		if !checker.Checker_isTypeAssignableTo(c, sigFnType, expected) {
			continue
		}
		// Concrete returns must also be assignable in the other direction so the
		// rewrite cannot change the expression's inferred type. Generic siblings are
		// accepted on the relation above alone: every *Unsafe sibling in the pinned
		// effect package mirrors its constructor's instantiation, and the real
		// exceptions are all rejected by the argument or relation checks.
		if len(sig.TypeParameters()) == 0 && !returnsMutuallyAssignable(c, c.GetReturnTypeOfSignature(sig), successType) {
			continue
		}
		return true
	}
	return false
}

// signatureAcceptsArguments reports whether the signature is applicable to the exact
// argument list of the constructor call: compatible arity and every argument type
// accepted by its parameter (falling back to the constraint for type parameters).
func signatureAcceptsArguments(tp *typeparser.TypeParser, c *checker.Checker, sig *checker.Signature, args []*ast.Node) bool {
	params := sig.Parameters()
	if len(args) < sig.MinArgumentCount() {
		return false
	}
	if len(args) > len(params) && !sig.HasRestParameter() {
		return false
	}
	for i, arg := range args {
		if sig.HasRestParameter() && i >= len(params)-1 {
			// Rest arguments are constrained by the whole-signature relation instead.
			break
		}
		if i >= len(params) {
			return false
		}
		paramType := c.GetTypeOfSymbol(params[i])
		argType := tp.GetTypeAtLocation(arg)
		if paramType == nil || argType == nil || !typeAcceptsArgumentValue(c, argType, paramType) {
			return false
		}
	}
	return true
}

// typeAcceptsArgumentValue reports whether a value of argType can be passed where
// paramType is expected, treating unconstrained type parameters as accepting anything.
func typeAcceptsArgumentValue(c *checker.Checker, argType *checker.Type, paramType *checker.Type) bool {
	if paramType.Flags()&checker.TypeFlagsTypeParameter != 0 {
		constraint := c.GetConstraintOfTypeParameter(paramType)
		return constraint == nil || typeAcceptsArgumentValue(c, argType, constraint)
	}
	return checker.Checker_isTypeAssignableTo(c, argType, paramType)
}

// returnsMutuallyAssignable reports whether a concrete sibling return type and the
// Effect success type denote the same value type: assignable in both directions, so
// the rewrite cannot narrow or widen the expression's inferred type.
func returnsMutuallyAssignable(c *checker.Checker, ret *checker.Type, successType *checker.Type) bool {
	if ret == nil {
		return false
	}
	return checker.Checker_isTypeAssignableTo(c, ret, successType) &&
		checker.Checker_isTypeAssignableTo(c, successType, ret)
}

// resolveAliasedSymbol follows import/export aliases to the original symbol.
func resolveAliasedSymbol(c *checker.Checker, sym *ast.Symbol) *ast.Symbol {
	for sym != nil && sym.Flags&ast.SymbolFlagsAlias != 0 {
		sym = c.GetAliasedSymbol(sym)
	}
	return sym
}
