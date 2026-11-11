package rules

import (
	"sort"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var MissingConcurrencyOption = rule.Rule{
	Name:            "missingConcurrencyOption",
	Group:           "style",
	Description:     "Reports Effect.all and Effect.forEach calls without an explicit concurrency option, which run sequentially by default",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_0_runs_sequentially_unless_concurrency_is_set_pass_concurrency_Colon_1_to_keep_sequential_execution_or_set_the_concurrency_you_intend_effect_missingConcurrencyOption.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMissingConcurrencyOption(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_0_runs_sequentially_unless_concurrency_is_set_pass_concurrency_Colon_1_to_keep_sequential_execution_or_set_the_concurrency_you_intend_effect_missingConcurrencyOption,
				nil,
				string(match.Api),
			)
		}
		return diags
	},
}

type ConcurrencyApi string

const (
	ConcurrencyApiAll     ConcurrencyApi = "all"
	ConcurrencyApiForEach ConcurrencyApi = "forEach"
)

type MissingConcurrencyOptionMatch struct {
	SourceFile     *ast.SourceFile
	Location       core.TextRange
	Api            ConcurrencyApi
	Flow           *typeparser.PipingFlow
	StepIndex      int
	OptionsLiteral *ast.Node
}

func AnalyzeMissingConcurrencyOption(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []MissingConcurrencyOptionMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []MissingConcurrencyOptionMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for index, step := range flow.Transformations().All() {
			callee := step.Callee()
			if !isInvokedCallee(callee) {
				continue
			}
			api, ok := concurrencyApiOf(tp, callee)
			if !ok {
				continue
			}
			options, ok := missingConcurrencyOptionsArgument(api, step)
			if !ok {
				continue
			}
			var literal *ast.Node
			if options != nil {
				literal = ast.SkipParentheses(options)
				if literal.Kind != ast.KindObjectLiteralExpression || objectLiteralStatesConcurrency(literal) {
					continue
				}
			}
			matches = append(matches, MissingConcurrencyOptionMatch{
				SourceFile:     sf,
				Location:       scanner.GetErrorRangeForNode(sf, callee),
				Api:            api,
				Flow:           flow,
				StepIndex:      index,
				OptionsLiteral: literal,
			})
		}
	}
	sort.SliceStable(matches, func(i, j int) bool {
		return matches[i].Location.Pos() < matches[j].Location.Pos()
	})
	return matches
}

func isInvokedCallee(callee *ast.Node) bool {
	return callee != nil && callee.Parent != nil && ast.IsCallExpression(callee.Parent) &&
		callee.Parent.AsCallExpression().Expression == callee
}

func concurrencyApiOf(tp *typeparser.TypeParser, callee *ast.Node) (ConcurrencyApi, bool) {
	for _, api := range []ConcurrencyApi{ConcurrencyApiForEach, ConcurrencyApiAll} {
		if tp.IsNodeReferenceToEffectModuleApi(callee, string(api)) {
			return api, true
		}
	}
	return "", false
}

func IsInputOnlyCall(step typeparser.PipingFlowTransformation) bool {
	return step.Kind() == typeparser.TransformationKindCall && step.Args().Len() == 0
}

const forEachOptionsIndex = 1

func missingConcurrencyOptionsArgument(api ConcurrencyApi, step typeparser.PipingFlowTransformation) (*ast.Node, bool) {
	args := step.Args()
	switch {
	case IsInputOnlyCall(step):
		return nil, true
	case step.Kind() == typeparser.TransformationKindCallWithOptionals:
		if args.Len() == 1 {
			return args.At(0), true
		}
	case api == ConcurrencyApiForEach && forEachTakesCallbackArgument(step.Kind()):
		switch {
		case args.Len() <= forEachOptionsIndex:
			return nil, true
		case args.Len() == forEachOptionsIndex+1:
			return args.At(forEachOptionsIndex), true
		}
	}
	return nil, false
}

func forEachTakesCallbackArgument(kind typeparser.TransformationKind) bool {
	switch kind {
	case typeparser.TransformationKindDataFirst,
		typeparser.TransformationKindPipe,
		typeparser.TransformationKindPipeable,
		typeparser.TransformationKindCall:
		return true
	}
	return false
}

func objectLiteralStatesConcurrency(literal *ast.Node) bool {
	for _, property := range literal.AsObjectLiteralExpression().Properties.Nodes {
		if property.Kind == ast.KindSpreadAssignment {
			return true
		}
		name := property.Name()
		if name == nil {
			continue
		}
		switch ast.GetPropertyNameForPropertyNameNode(name) {
		case "concurrency", ast.InternalSymbolNameMissing:
			return true
		}
	}
	return false
}
