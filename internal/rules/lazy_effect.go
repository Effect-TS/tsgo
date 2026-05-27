package rules

import (
	"fmt"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var LazyEffect = rule.Rule{
	Name:            "lazyEffect",
	Group:           "antipattern",
	Description:     "Suggests avoiding exported zero-argument functions and service members that lazily return Effect or Stream values",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.X_0_returns_a_lazy_1_1_is_already_lazy_so_wrapping_it_in_a_zero_argument_function_adds_unnecessary_indirection_effect_lazyEffect.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		if !strings.HasPrefix(ctx.TypeParser.DetectEffectVersionString(), "4.") {
			return nil
		}

		var diags []*ast.Diagnostic

		for _, stmt := range ctx.SourceFile.Statements.Nodes {
			if stmt == nil {
				continue
			}

			diags = append(diags, checkLazyEffectExport(ctx, stmt)...)
			diags = append(diags, checkLazyEffectInterface(ctx, stmt)...)
			diags = append(diags, checkLazyEffectService(ctx, stmt)...)
		}

		return diags
	},
}

func checkLazyEffectInterface(ctx *rule.Context, stmt *ast.Node) []*ast.Diagnostic {
	if stmt.Kind != ast.KindInterfaceDeclaration || !ast.HasSyntacticModifier(stmt, ast.ModifierFlagsExport) || stmt.Name() == nil {
		return nil
	}

	interfaceName := scanner.GetTextOfNode(stmt.Name())
	var diags []*ast.Diagnostic
	for _, member := range stmt.AsInterfaceDeclaration().Members.Nodes {
		if member == nil || member.Name() == nil {
			continue
		}
		if member.Kind != ast.KindPropertySignature && member.Kind != ast.KindMethodSignature {
			continue
		}

		memberType := ctx.TypeParser.GetTypeAtLocation(member.Name())
		lazyTypeName, ok := lazyEffectLikeTypeName(ctx.Checker, ctx.TypeParser, memberType, member.Name())
		if !ok {
			continue
		}

		messageSubject := fmt.Sprintf("Interface '%s' member '%s'", interfaceName, scanner.GetTextOfNode(member.Name()))
		diags = append(diags, ctx.NewDiagnostic(
			ctx.SourceFile,
			ctx.GetErrorRange(member.Name()),
			tsdiag.X_0_returns_a_lazy_1_1_is_already_lazy_so_wrapping_it_in_a_zero_argument_function_adds_unnecessary_indirection_effect_lazyEffect,
			nil,
			messageSubject,
			lazyTypeName,
		))
	}

	return diags
}

func checkLazyEffectExport(ctx *rule.Context, stmt *ast.Node) []*ast.Diagnostic {
	if !ast.HasSyntacticModifier(stmt, ast.ModifierFlagsExport) {
		return nil
	}

	switch stmt.Kind {
	case ast.KindFunctionDeclaration:
		if stmt.Body() == nil || stmt.Name() == nil || stmt.Name().Kind != ast.KindIdentifier {
			return nil
		}
		return lazyEffectDiagnosticForExportedDeclaration(ctx, stmt.Name())

	case ast.KindVariableStatement:
		declList := stmt.AsVariableStatement().DeclarationList
		if declList == nil {
			return nil
		}

		var diags []*ast.Diagnostic
		for _, decl := range declList.AsVariableDeclarationList().Declarations.Nodes {
			if decl == nil || decl.Name() == nil || decl.Name().Kind != ast.KindIdentifier {
				continue
			}
			diags = append(diags, lazyEffectDiagnosticForExportedDeclaration(ctx, decl.Name())...)
		}
		return diags
	}

	return nil
}

func lazyEffectDiagnosticForExportedDeclaration(ctx *rule.Context, name *ast.Node) []*ast.Diagnostic {
	declType := ctx.TypeParser.GetTypeAtLocation(name)
	lazyTypeName, ok := lazyEffectLikeTypeName(ctx.Checker, ctx.TypeParser, declType, name)
	if !ok {
		return nil
	}

	messageSubject := fmt.Sprintf("Exported declaration '%s'", scanner.GetTextOfNode(name))
	return []*ast.Diagnostic{ctx.NewDiagnostic(
		ctx.SourceFile,
		ctx.GetErrorRange(name),
		tsdiag.X_0_returns_a_lazy_1_1_is_already_lazy_so_wrapping_it_in_a_zero_argument_function_adds_unnecessary_indirection_effect_lazyEffect,
		nil,
		messageSubject,
		lazyTypeName,
	)}
}

func checkLazyEffectService(ctx *rule.Context, stmt *ast.Node) []*ast.Diagnostic {
	if stmt.Kind != ast.KindClassDeclaration || stmt.Name() == nil || stmt.Name().Kind != ast.KindIdentifier {
		return nil
	}
	if ctx.TypeParser.ExtendsContextService(stmt) == nil {
		return nil
	}

	classSym := ctx.Checker.GetSymbolAtLocation(stmt.Name())
	if classSym == nil {
		return nil
	}
	classType := ctx.Checker.GetTypeOfSymbolAtLocation(classSym, stmt.Name())
	if classType == nil {
		return nil
	}

	service := ctx.TypeParser.ServiceType(classType, stmt.Name())
	if service == nil || service.Shape == nil {
		return nil
	}

	serviceName := scanner.GetTextOfNode(stmt.Name())
	var diags []*ast.Diagnostic
	for _, member := range ctx.Checker.GetPropertiesOfType(service.Shape) {
		if member == nil {
			continue
		}

		memberType := ctx.Checker.GetTypeOfSymbolAtLocation(member, stmt.Name())
		lazyTypeName, ok := lazyEffectLikeTypeName(ctx.Checker, ctx.TypeParser, memberType, stmt.Name())
		if !ok {
			continue
		}

		messageSubject := fmt.Sprintf("Service '%s' member '%s'", serviceName, member.Name)
		diags = append(diags, ctx.NewDiagnostic(
			ctx.SourceFile,
			ctx.GetErrorRange(stmt.Name()),
			tsdiag.X_0_returns_a_lazy_1_1_is_already_lazy_so_wrapping_it_in_a_zero_argument_function_adds_unnecessary_indirection_effect_lazyEffect,
			nil,
			messageSubject,
			lazyTypeName,
		))
	}

	return diags
}

func lazyEffectLikeTypeName(c *checker.Checker, tp *typeparser.TypeParser, t *checker.Type, atLocation *ast.Node) (string, bool) {
	if c == nil || tp == nil || t == nil {
		return "", false
	}

	callSignatures := c.GetSignaturesOfType(t, checker.SignatureKindCall)
	if len(callSignatures) != 1 {
		return "", false
	}

	sig := callSignatures[0]
	if len(sig.TypeParameters()) != 0 {
		return "", false
	}
	if len(sig.Parameters()) != 0 {
		return "", false
	}

	returnType := c.GetReturnTypeOfSignature(sig)
	if returnType == nil {
		return "", false
	}

	if tp.StrictIsEffectType(returnType, atLocation) {
		return "Effect", true
	}
	if tp.LayerType(returnType, atLocation) != nil {
		return "Layer", true
	}
	if tp.StreamType(returnType, atLocation) != nil {
		return "Stream", true
	}

	return "", false
}
