package fixables

import (
	"sort"
	"strconv"
	"strings"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var MissingEffectErrorCatchFix = fixable.Fixable{
	Name:        "missingEffectErrorCatch",
	Description: "Catch all errors with Effect.catch/catchAll",
	ErrorCodes:  []int32{tsdiag.Missing_errors_0_in_the_expected_Effect_type_effect_missingEffectError.Code()},
	FixIDs:      []string{"missingEffectError_catch", "missingEffectError_catchAll", "missingEffectError_tagged"},
	Run:         runMissingEffectErrorCatchFix,
}

func runMissingEffectErrorCatchFix(ctx *fixable.Context) []ls.CodeAction {
	c := ctx.Checker

	sf := ctx.SourceFile

	type fixCandidate struct {
		start int
		end   int
		tags  []string
	}

	var catchCandidate *fixCandidate
	var taggedCandidate *fixCandidate

	matches := rules.AnalyzeMissingEffectError(c, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		errorExpr := expressionNodeForMissingEffectError(match.ErrorNode)
		if errorExpr == nil {
			continue
		}

		nodeStartPos := scanner.GetTokenPosOfNode(errorExpr, sf, false)
		nodeEndPos := errorExpr.End()

		// Offer catch/catchAll only when expected error type is `never`.
		if match.ExpectedErrorType.Flags()&checker.TypeFlagsNever != 0 {
			if catchCandidate == nil || nodeStartPos < catchCandidate.start || (nodeStartPos == catchCandidate.start && nodeEndPos < catchCandidate.end) {
				catchCandidate = &fixCandidate{
					start: nodeStartPos,
					end:   nodeEndPos,
				}
			}
		}

		// Offer catchTags only when all missing error members have a literal _tag.
		if tags, ok := collectLiteralMissingErrorTags(c, errorExpr, match.UnhandledErrors); ok {
			if taggedCandidate == nil || nodeStartPos < taggedCandidate.start || (nodeStartPos == taggedCandidate.start && nodeEndPos < taggedCandidate.end) {
				taggedCandidate = &fixCandidate{
					start: nodeStartPos,
					end:   nodeEndPos,
					tags:  tags,
				}
			}
		}
	}

	var actions []ls.CodeAction

	if catchCandidate != nil {
		methodName := "catch"
		if typeparser.SupportedEffectVersion(c) == typeparser.EffectMajorV3 {
			methodName = "catchAll"
		}

		description := "Catch all errors with Effect." + methodName
		todoMessage := "TODO: " + methodName + " not implemented"

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: description,
			Run: func(tracker *change.Tracker) {
				tracker.InsertText(sf, ctx.BytePosToLSPPosition(catchCandidate.start), "Effect."+methodName+"(")
				tracker.InsertText(sf, ctx.BytePosToLSPPosition(catchCandidate.end), ", () => Effect.dieMessage("+strconv.Quote(todoMessage)+"))")
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	if taggedCandidate != nil {
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Catch unexpected errors with Effect.catchTag",
			Run: func(tracker *change.Tracker) {
				tracker.InsertText(sf, ctx.BytePosToLSPPosition(taggedCandidate.start), "Effect.catchTags(")
				tracker.InsertText(sf, ctx.BytePosToLSPPosition(taggedCandidate.end), ", "+buildCatchTagsHandlersObject(taggedCandidate.tags)+")")
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	return actions
}

func collectLiteralMissingErrorTags(c *checker.Checker, atLocation *ast.Node, missing []*checker.Type) ([]string, bool) {
	if len(missing) == 0 {
		return nil, false
	}

	seen := make(map[string]bool)
	tags := make([]string, 0, len(missing))
	for _, missingType := range missing {
		tagProp := c.GetPropertyOfType(missingType, "_tag")
		if tagProp == nil {
			tagProp = typeparser.GetPropertyOfTypeByName(c, missingType, "_tag")
		}
		if tagProp == nil {
			return nil, false
		}
		tagType := c.GetTypeOfSymbolAtLocation(tagProp, atLocation)
		if tagType == nil || tagType.Flags()&checker.TypeFlagsStringLiteral == 0 {
			return nil, false
		}
		tagValue, ok := tagType.AsLiteralType().Value().(string)
		if !ok {
			return nil, false
		}
		if !seen[tagValue] {
			seen[tagValue] = true
			tags = append(tags, tagValue)
		}
	}

	sort.Strings(tags)
	return tags, true
}

func buildCatchTagsHandlersObject(tags []string) string {
	var b strings.Builder
	b.WriteString("{ ")
	for i, tag := range tags {
		if i > 0 {
			b.WriteString(", ")
		}
		b.WriteString(strconv.Quote(tag))
		b.WriteString(": () => Effect.dieMessage(")
		b.WriteString(strconv.Quote("TODO: catchTags() not implemented for " + tag))
		b.WriteString(")")
	}
	b.WriteString(" }")
	return b.String()
}

func expressionNodeForMissingEffectError(node *ast.Node) *ast.Node {
	if node == nil {
		return nil
	}
	// Handle SatisfiesExpression before the general IsExpression check:
	// we want to wrap just the LHS expression, not the entire `expr satisfies Type`.
	if node.Kind == ast.KindSatisfiesExpression {
		if satExpr := node.AsSatisfiesExpression(); satExpr != nil && satExpr.Expression != nil {
			return satExpr.Expression
		}
		return nil
	}
	if ast.IsExpression(node) {
		return node
	}
	switch node.Kind {
	case ast.KindVariableDeclaration:
		if decl := node.AsVariableDeclaration(); decl != nil && decl.Initializer != nil && ast.IsExpression(decl.Initializer) {
			return decl.Initializer
		}
	case ast.KindReturnStatement:
		if stmt := node.AsReturnStatement(); stmt != nil && ast.IsExpression(stmt.Expression) {
			return stmt.Expression
		}
	case ast.KindArrowFunction:
		if fn := node.AsArrowFunction(); fn != nil && ast.IsExpression(fn.Body) {
			return fn.Body
		}
	}
	return nil
}
