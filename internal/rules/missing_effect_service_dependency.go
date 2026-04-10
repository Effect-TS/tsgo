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
)

// MissingEffectServiceDependency checks that Effect.Service dependencies satisfy
// all required layer inputs. It detects when a class extending Effect.Service has
// required services (from the layer's RIn) that are not provided in the dependencies
// configuration option. V3-only, default severity off.
var MissingEffectServiceDependency = rule.Rule{
	Name:            "missingEffectServiceDependency",
	Group:           "style",
	Description:     "Checks that Effect.Service dependencies satisfy all required layer inputs",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3"},
	Codes: []int32{
		tsdiag.Service_0_is_required_but_not_provided_by_dependencies_effect_missingEffectServiceDependency.Code(),
		tsdiag.Services_0_are_required_but_not_provided_by_dependencies_effect_missingEffectServiceDependency.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		// V3-only rule
		if ctx.TypeParser.SupportedEffectVersion() != typeparser.EffectMajorV3 {
			return nil
		}

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

			if node.Kind == ast.KindClassDeclaration {
				if d := checkServiceDependencies(ctx, node); len(d) > 0 {
					diags = append(diags, d...)
					continue // skip children
				}
			}

			// Enqueue children
			node.ForEachChild(pushChild)
		}

		return diags
	},
}

// checkServiceDependencies checks if a class extending Effect.Service has all
// required service dependencies satisfied.
func checkServiceDependencies(ctx *rule.Context, node *ast.Node) []*ast.Diagnostic {
	// Check if this class extends Effect.Service
	serviceResult := ctx.TypeParser.ExtendsEffectV3Service(node)
	if serviceResult == nil {
		return nil
	}

	className := serviceResult.ClassName
	options := serviceResult.Options

	// Get the class symbol and type
	classSym := ctx.Checker.GetSymbolAtLocation(className)
	if classSym == nil {
		return nil
	}
	classType := ctx.Checker.GetTypeOfSymbolAtLocation(classSym, node)
	if classType == nil {
		return nil
	}

	// Try DefaultWithoutDependencies first, then fall back to Default
	defaultProp := ctx.Checker.GetPropertyOfType(classType, "DefaultWithoutDependencies")
	if defaultProp == nil {
		defaultProp = ctx.Checker.GetPropertyOfType(classType, "Default")
	}
	if defaultProp == nil {
		return nil
	}

	defaultType := ctx.Checker.GetTypeOfSymbolAtLocation(defaultProp, node)
	if defaultType == nil {
		return nil
	}

	// Parse as Layer type to get RIn
	layer := ctx.TypeParser.LayerType(defaultType, node)
	if layer == nil {
		return nil
	}

	// Use a shared memory map for both required and provided services
	servicesMemory := make(map[string]*checker.Type)
	excludeNever := func(t *checker.Type) bool {
		return t.Flags()&checker.TypeFlagsNever != 0
	}

	// Get all required service indexes from RIn
	requiredResult := ctx.TypeParser.AppendToUniqueTypesMap(servicesMemory, layer.RIn, excludeNever)
	requiredIndexes := requiredResult.AllIndexes

	if len(requiredIndexes) == 0 {
		return nil
	}

	// Process dependencies to find provided services
	providedIndexes := make(map[string]bool)

	if options != nil {
		optionsType := ctx.TypeParser.GetTypeAtLocation(options)
		if optionsType != nil {
			dependenciesProp := ctx.Checker.GetPropertyOfType(optionsType, "dependencies")
			if dependenciesProp != nil {
				dependenciesType := ctx.Checker.GetTypeOfSymbolAtLocation(dependenciesProp, options)
				if dependenciesType != nil {
					// Get the number index type to extract individual dependency types
					numberIndexType := ctx.Checker.GetNumberIndexType(dependenciesType)
					if numberIndexType != nil {
						depTypes := ctx.TypeParser.UnrollUnionMembers(numberIndexType)
						for _, depType := range depTypes {
							// Parse each dependency as Layer type
							depLayer := ctx.TypeParser.LayerType(depType, options)
							if depLayer != nil {
								// Add the ROut of this dependency to provided services
								providedResult := ctx.TypeParser.AppendToUniqueTypesMap(servicesMemory, depLayer.ROut, excludeNever)
								for _, idx := range providedResult.AllIndexes {
									providedIndexes[idx] = true
								}
							}
						}
					}
				}
			}
		}
	}

	// Find missing indexes: required but not provided
	var missingIndexes []string
	for _, idx := range requiredIndexes {
		if !providedIndexes[idx] {
			missingIndexes = append(missingIndexes, idx)
		}
	}

	if len(missingIndexes) == 0 {
		return nil
	}

	// Build the diagnostic
	var missingTypeNames []string
	for _, idx := range missingIndexes {
		t := servicesMemory[idx]
		if t != nil {
			missingTypeNames = append(missingTypeNames, ctx.Checker.TypeToString(t))
		}
	}

	if len(missingTypeNames) == 0 {
		return nil
	}

	if len(missingTypeNames) == 1 {
		// Singular
		diag := ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(className), tsdiag.Service_0_is_required_but_not_provided_by_dependencies_effect_missingEffectServiceDependency, nil, missingTypeNames[0])
		return []*ast.Diagnostic{diag}
	}

	// Plural: format as 'X', 'Y'
	var quotedNames []string
	for _, name := range missingTypeNames {
		quotedNames = append(quotedNames, fmt.Sprintf("'%s'", name))
	}
	formatted := strings.Join(quotedNames, ", ")
	diag := ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(className), tsdiag.Services_0_are_required_but_not_provided_by_dependencies_effect_missingEffectServiceDependency, nil, formatted)
	return []*ast.Diagnostic{diag}
}
