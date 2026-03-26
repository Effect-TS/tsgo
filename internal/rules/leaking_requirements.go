package rules

import (
	"sort"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// LeakingRequirements detects when service methods inadvertently expose implementation
// dependencies (Requirements) in their public type signatures. When every method of a
// service requires the same dependency types from callers, this is typically a sign that
// those dependencies should be resolved at Layer creation time instead.
// Supports both V3 and V4, default severity suggestion.
var LeakingRequirements = rule.Rule{
	Name:            "leakingRequirements",
	Group:           "antipattern",
	Description:     "Detects implementation services leaked in service methods",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Methods_of_this_Service_require_0_from_every_caller_This_leaks_implementation_details_resolve_these_dependencies_at_Layer_creation_time_effect_leakingRequirements.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		// Stack-based traversal
		nodeToVisit := make([]*ast.Node, 0)
		pushChild := func(child *ast.Node) bool {
			nodeToVisit = append(nodeToVisit, child)
			return false
		}
		ctx.SourceFile.AsNode().ForEachChild(pushChild)

		for len(nodeToVisit) > 0 {
			node := nodeToVisit[len(nodeToVisit)-1]
			nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

			// Collect types to check and the node to report on
			type typeToCheck struct {
				t          *checker.Type
				reportNode *ast.Node
			}
			var typesToCheck []typeToCheck

			switch node.Kind {
			case ast.KindCallExpression:
				call := node.AsCallExpression()
				if call.Expression != nil && call.Expression.Kind == ast.KindPropertyAccessExpression {
					propAccess := call.Expression.AsPropertyAccessExpression()
					if propAccess.Name() != nil && propAccess.Name().Kind == ast.KindIdentifier {
						name := scanner.GetTextOfNode(propAccess.Name())
						if name == "GenericTag" || name == "Service" {
							nodeType := typeparser.GetTypeAtLocation(ctx.Checker, node)
							if nodeType != nil {
								typesToCheck = append(typesToCheck, typeToCheck{t: nodeType, reportNode: node})
							}
						}
					}
				}
			case ast.KindClassDeclaration:
				if node.Name() != nil && node.AsClassDeclaration().HeritageClauses != nil {
					classSym := ctx.Checker.GetSymbolAtLocation(node.Name())
					if classSym != nil {
						classType := ctx.Checker.GetTypeOfSymbolAtLocation(classSym, node)
						if classType != nil {
							typesToCheck = append(typesToCheck, typeToCheck{t: classType, reportNode: node.Name()})
						}
					}
				}
			}

			if len(typesToCheck) == 0 {
				// No patterns matched, enqueue children
				node.ForEachChild(pushChild)
				continue
			}

			// Check each collected type
			matched := false
			for _, ttc := range typesToCheck {
				// Try ContextTag first, fall back to ServiceType
				service := typeparser.ContextTag(ctx.Checker, ttc.t, node)
				if service == nil {
					service = typeparser.ServiceType(ctx.Checker, ttc.t, node)
				}
				if service == nil {
					continue
				}

				leaked := parseLeakedRequirements(ctx.Checker, service.Shape, node)
				if len(leaked) > 0 {
					leaked = filterExpectedLeakingRequirements(ctx.Checker, ttc.reportNode, leaked)
				}
				if len(leaked) > 0 {
					matched = true

					// Sort deterministically by type name (alphabetical)
					sort.Slice(leaked, func(i, j int) bool {
						return ctx.Checker.TypeToString(leaked[i]) < ctx.Checker.TypeToString(leaked[j])
					})

					// Format as "TypeA | TypeB | TypeC"
					var typeNames []string
					for _, t := range leaked {
						typeNames = append(typeNames, ctx.Checker.TypeToString(t))
					}
					formatted := strings.Join(typeNames, " | ")

					diags = append(diags, ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(ttc.reportNode), tsdiag.Methods_of_this_Service_require_0_from_every_caller_This_leaks_implementation_details_resolve_these_dependencies_at_Layer_creation_time_effect_leakingRequirements, nil, formatted))
				} else {
					matched = true
				}
			}

			if !matched {
				// Type resolution failed for all candidates, continue visiting children
				node.ForEachChild(pushChild)
			}
		}

		return diags
	},
}

func filterExpectedLeakingRequirements(c *checker.Checker, reportNode *ast.Node, leaked []*checker.Type) []*checker.Type {
	if reportNode == nil || len(leaked) == 0 {
		return leaked
	}

	filtered := leaked[:0]
	for _, leakedType := range leaked {
		if leakedType == nil {
			continue
		}
		if isExpectedLeakingServiceSuppressed(c, reportNode, c.TypeToString(leakedType)) {
			continue
		}
		filtered = append(filtered, leakedType)
	}

	return filtered
}

func isExpectedLeakingServiceSuppressed(c *checker.Checker, startNode *ast.Node, leakedServiceName string) bool {
	if c == nil || startNode == nil || leakedServiceName == "" {
		return false
	}

	sourceFile := ast.GetSourceFileOfNode(startNode)
	if sourceFile == nil {
		return false
	}

	return ast.FindAncestorOrQuit(startNode, func(current *ast.Node) ast.FindAncestorResult {
		if current == nil {
			return ast.FindAncestorFalse
		}

		if hasExpectedLeakingComment(sourceFile.Text(), current.Pos(), leakedServiceName) {
			return ast.FindAncestorTrue
		}

		if ast.IsClassDeclaration(current) || ast.IsVariableStatement(current) || ast.IsExpressionStatement(current) || ast.IsStatement(current) {
			return ast.FindAncestorQuit
		}

		return ast.FindAncestorFalse
	}) != nil
}

func hasExpectedLeakingComment(sourceText string, pos int, leakedServiceName string) bool {
	if sourceText == "" || leakedServiceName == "" || pos < 0 || pos > len(sourceText) {
		return false
	}

	for commentRange := range scanner.GetLeadingCommentRanges(&ast.NodeFactory{}, sourceText, pos) {
		start := commentRange.Pos()
		end := commentRange.End()
		if start < 0 || end < 0 || start >= end || end > len(sourceText) {
			continue
		}

		commentText := sourceText[start:end]
		for line := range strings.SplitSeq(commentText, "\n") {
			_, suffix, found := strings.Cut(line, "@effect-expect-leaking")
			if !found {
				continue
			}
			if strings.Contains(suffix, leakedServiceName) {
				return true
			}
		}
	}

	return false
}

// parseLeakedRequirements analyzes the service shape to find requirement types that
// are shared across all effect-typed members. This is the "leaking requirements" heuristic.
func parseLeakedRequirements(c *checker.Checker, serviceShape *checker.Type, atLocation *ast.Node) []*checker.Type {
	properties := c.GetPropertiesOfType(serviceShape)
	if len(properties) < 1 {
		return nil
	}

	memory := make(map[string]*checker.Type)
	var sharedRequirementsKeys []string
	sharedInitialized := false
	effectMembers := 0

	shouldExclude := func(t *checker.Type) bool {
		// Exclude never
		if t.Flags()&checker.TypeFlagsNever != 0 {
			return true
		}
		// Exclude Scope types
		if typeparser.IsScopeType(c, t, atLocation) {
			return true
		}
		return false
	}

	for _, property := range properties {
		if property == nil {
			continue
		}

		servicePropertyType := c.GetTypeOfSymbolAtLocation(property, atLocation)
		if servicePropertyType == nil {
			continue
		}

		// Try to get the Effect's R type - either directly from the property type
		// or from the return type of a single call signature
		var effectContextType *checker.Type

		effect := typeparser.EffectType(c, servicePropertyType, atLocation)
		if effect != nil {
			effectContextType = effect.R
		} else {
			// Try call signature: if exactly 1 call signature, parse return type as Effect
			sigs := c.GetSignaturesOfType(servicePropertyType, checker.SignatureKindCall)
			if len(sigs) == 1 {
				retType := c.GetReturnTypeOfSignature(sigs[0])
				if retType != nil {
					retEffect := typeparser.EffectType(c, retType, atLocation)
					if retEffect != nil {
						effectContextType = retEffect.R
					}
				}
			}
		}

		if effectContextType == nil {
			continue
		}

		effectMembers++
		result := typeparser.AppendToUniqueTypesMap(c, memory, effectContextType, shouldExclude)

		if !sharedInitialized {
			sharedRequirementsKeys = result.AllIndexes
			sharedInitialized = true
		} else {
			// Intersect with current keys
			sharedRequirementsKeys = intersectStringSlices(sharedRequirementsKeys, result.AllIndexes)
			if len(sharedRequirementsKeys) == 0 {
				return nil
			}
		}
	}

	// Need at least 2 effect members for the heuristic
	if !sharedInitialized || len(sharedRequirementsKeys) == 0 || effectMembers < 2 {
		return nil
	}

	// Collect the shared requirement types, filtering out those with @effect-leakable-service
	var leaked []*checker.Type
	for _, key := range sharedRequirementsKeys {
		t := memory[key]
		if t == nil {
			continue
		}
		if hasLeakableServiceAnnotation(c, t) {
			continue
		}
		leaked = append(leaked, t)
	}

	return leaked
}

// hasLeakableServiceAnnotation checks if a type's declarations contain the
// @effect-leakable-service JSDoc annotation.
func hasLeakableServiceAnnotation(c *checker.Checker, t *checker.Type) bool {
	sym := t.Symbol()
	if sym == nil {
		return false
	}

	// Resolve aliases
	if sym.Flags&ast.SymbolFlagsAlias != 0 {
		resolved := c.GetAliasedSymbol(sym)
		if resolved != nil {
			sym = resolved
		}
	}

	for _, decl := range sym.Declarations {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil {
			continue
		}
		text := sf.Text()
		start := decl.Pos()
		end := decl.End()
		if start < 0 || end < 0 || start >= end || end > len(text) {
			continue
		}
		snippet := strings.ToLower(text[start:end])
		if strings.Contains(snippet, "@effect-leakable-service") {
			return true
		}
	}

	return false
}

// intersectStringSlices returns the elements that appear in both slices.
func intersectStringSlices(a, b []string) []string {
	set := make(map[string]bool, len(b))
	for _, s := range b {
		set[s] = true
	}
	var result []string
	for _, s := range a {
		if set[s] {
			result = append(result, s)
		}
	}
	return result
}
