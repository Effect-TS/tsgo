package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

var MissingPipeableSignature = rule.Rule{
	Name:            "missingPipeableSignature",
	Group:           "style",
	Description:     "Reports exported fixed-arity functions whose call signatures have no corresponding pipeable overload",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Exported_function_0_has_no_pipeable_overload_corresponding_to_its_signature_1_effect_missingPipeableSignature.Code(),
	},
	Run: checkMissingPipeableSignatures,
}

func checkMissingPipeableSignatures(ctx *rule.Context) []*ast.Diagnostic {
	moduleSymbol := checker.Checker_getSymbolOfDeclaration(ctx.Checker, ctx.SourceFile.AsNode())
	if moduleSymbol == nil {
		return nil
	}

	var diagnostics []*ast.Diagnostic
	for _, exportSymbol := range ctx.Checker.GetExportsOfModule(moduleSymbol) {
		target, location := localExportTarget(ctx, exportSymbol)
		if target == nil || location == nil {
			continue
		}

		exportType := ctx.Checker.GetTypeOfSymbolAtLocation(target, location)
		if exportType == nil {
			continue
		}
		signatures := ctx.Checker.GetSignaturesOfType(exportType, checker.SignatureKindCall)
		if len(signatures) == 0 {
			continue
		}

		pipeableTargets := make(map[*checker.Signature]bool)
		hasPipeableSignature := make(map[*checker.Signature]bool)
		for _, dataFirst := range signatures {
			if !isEligibleDataFirstSignature(dataFirst) {
				continue
			}
			params := dataFirst.Parameters()
			for _, subjectIndex := range []int{0, len(params) - 1} {
				for _, candidate := range signatures {
					if candidate == dataFirst {
						continue
					}
					if typeparser.MatchesPipeableSignature(ctx.Checker, dataFirst, candidate, subjectIndex, nil) {
						hasPipeableSignature[dataFirst] = true
						pipeableTargets[candidate] = true
					}
				}
			}
		}

		for _, signature := range signatures {
			if !isEligibleDataFirstSignature(signature) || pipeableTargets[signature] || hasPipeableSignature[signature] {
				continue
			}
			diagnostics = append(diagnostics, ctx.NewDiagnostic(
				ctx.SourceFile,
				ctx.GetErrorRange(location),
				tsdiag.Exported_function_0_has_no_pipeable_overload_corresponding_to_its_signature_1_effect_missingPipeableSignature,
				nil,
				exportSymbol.Name,
				ctx.Checker.SignatureToStringEx(signature, location, checker.TypeFormatFlagsWriteArrowStyleSignature, nil),
			))
		}
	}

	return diagnostics
}

func localExportTarget(ctx *rule.Context, exportSymbol *ast.Symbol) (*ast.Symbol, *ast.Node) {
	if exportSymbol == nil {
		return nil, nil
	}

	target := exportSymbol
	if target.Flags&ast.SymbolFlagsAlias != 0 {
		target = ctx.Checker.GetAliasedSymbol(target)
	}
	if target == nil {
		return nil, nil
	}

	var targetDeclaration *ast.Node
	for _, declaration := range target.Declarations {
		if declaration != nil && ast.GetSourceFileOfNode(declaration) == ctx.SourceFile {
			targetDeclaration = declaration
			break
		}
	}
	if targetDeclaration == nil {
		return nil, nil
	}

	location := ast.GetNameOfDeclaration(target.ValueDeclaration)
	if location == nil {
		location = ast.GetNameOfDeclaration(targetDeclaration)
	}
	if location == nil {
		location = targetDeclaration
	}
	return target, location
}

func isEligibleDataFirstSignature(signature *checker.Signature) bool {
	return signature != nil && !signature.HasRestParameter() && len(signature.Parameters()) >= 2
}
