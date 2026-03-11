package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var ExtendsNativeError = rule.Rule{
	Name:            "extendsNativeError",
	Description:     "Warns when a class directly extends the native Error class",
	DefaultSeverity: etscore.SeverityOff,
	Codes:           []int32{tsdiag.Avoid_extending_the_native_Error_class_directly_Consider_using_a_tagged_error_e_g_Data_TaggedError_to_maintain_type_safety_in_the_Effect_failure_channel_effect_extendsNativeError.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeExtendsNativeError(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.Location, tsdiag.Avoid_extending_the_native_Error_class_directly_Consider_using_a_tagged_error_e_g_Data_TaggedError_to_maintain_type_safety_in_the_Effect_failure_channel_effect_extendsNativeError, nil)
		}
		return diags
	},
}

type ExtendsNativeErrorMatch struct {
	Location core.TextRange
}

func AnalyzeExtendsNativeError(c *checker.Checker, sf *ast.SourceFile) []ExtendsNativeErrorMatch {
	errorSymbol := c.ResolveName("Error", nil, ast.SymbolFlagsType, false)
	if errorSymbol == nil {
		return nil
	}

	var matches []ExtendsNativeErrorMatch

	nodeToVisit := make([]*ast.Node, 0)
	for child := range sf.AsNode().IterChildren() {
		nodeToVisit = append(nodeToVisit, child)
	}

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		if node.Kind == ast.KindClassDeclaration {
			if m := checkExtendsNativeError(c, sf, node, errorSymbol); m != nil {
				matches = append(matches, *m)
			}
		}

		for child := range node.IterChildren() {
			nodeToVisit = append(nodeToVisit, child)
		}
	}

	return matches
}

func checkExtendsNativeError(c *checker.Checker, sf *ast.SourceFile, node *ast.Node, errorSymbol *ast.Symbol) *ExtendsNativeErrorMatch {
	extendsElements := ast.GetExtendsHeritageClauseElements(node)
	if len(extendsElements) == 0 {
		return nil
	}

	for _, elem := range extendsElements {
		if elem.Kind != ast.KindExpressionWithTypeArguments {
			continue
		}
		expr := elem.AsExpressionWithTypeArguments().Expression

		exprSymbol := c.GetSymbolAtLocation(expr)
		resolvedSymbol := exprSymbol
		if resolvedSymbol != nil && resolvedSymbol.Flags&ast.SymbolFlagsAlias != 0 {
			resolvedSymbol = c.GetAliasedSymbol(resolvedSymbol)
		}

		isNativeError := resolvedSymbol == errorSymbol
		if !isNativeError && resolvedSymbol != nil && resolvedSymbol != errorSymbol {
			exprType := c.GetTypeAtLocation(expr)
			if exprType != nil {
				constructSignatures := c.GetSignaturesOfType(exprType, checker.SignatureKindConstruct)
				if len(constructSignatures) > 0 {
					instanceType := c.GetReturnTypeOfSignature(constructSignatures[0])
					if instanceType != nil && instanceType.Symbol() == errorSymbol {
						isNativeError = true
					}
				}
			}
		}

		if isNativeError {
			locationNode := node.Name()
			if locationNode == nil {
				locationNode = expr
			}
			return &ExtendsNativeErrorMatch{
				Location: scanner.GetErrorRangeForNode(sf, locationNode),
			}
		}
	}

	return nil
}
