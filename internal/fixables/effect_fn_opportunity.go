package fixables

import (
	"strconv"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/astnav"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var EffectFnOpportunityFix = fixable.Fixable{
	Name:        "effectFnOpportunity",
	Description: "Convert to Effect.fn",
	ErrorCodes:  []int32{tsdiag.This_expression_can_be_rewritten_in_the_reusable_function_form_0_effect_effectFnOpportunity.Code()},
	FixIDs: []string{
		"effectFnOpportunity_toEffectFnWithSpan",
		"effectFnOpportunity_toEffectFnUntraced",
		"effectFnOpportunity_toEffectFnNoSpan",
		"effectFnOpportunity_toEffectFnSpanInferred",
		"effectFnOpportunity_toEffectFnSpanSuggested",
	},
	Run: runEffectFnOpportunityFix,
}

func runEffectFnOpportunityFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	effectConfig := ctx.Options

	matches := rules.AnalyzeEffectFnOpportunity(ctx.TypeParser, c, sf)

	var result *typeparser.EffectFnOpportunityResult
	for _, match := range matches {
		diagRange := match.Location
		if diagRange.Intersects(ctx.Span) || ctx.Span.ContainedBy(diagRange) {
			result = match.Result
			break
		}
	}
	if result == nil {
		return nil
	}
	isFuncDecl := result.TargetNode.Kind == ast.KindFunctionDeclaration

	var actions []ls.CodeAction

	// Fix 1: toEffectFnWithSpan - available when explicit withSpan expression exists
	if effectConfig.EffectFnIncludes(etscore.EffectFnSpan) && result.ExplicitTraceExpression != nil {
		// Remove withSpan from pipe args (it's the last one)
		pipeArgs := result.PipeArguments
		if len(pipeArgs) > 0 {
			pipeArgs = pipeArgs[:len(pipeArgs)-1]
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Effect.fn (with span from withSpan)",
			Run: func(tracker *rewriter.Tracker) {
				trace := scanner.GetTextOfNode(result.ExplicitTraceExpression)
				effectFnBuildReplacement(tracker, sf, result, "fn", trace, pipeArgs, isFuncDecl)
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	// Fix 2: toEffectFnUntraced - available when gen opportunity (generator function exists)
	if effectConfig.EffectFnIncludes(etscore.EffectFnUntraced) && result.GeneratorFunction != nil {
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Effect.fnUntraced",
			Run: func(tracker *rewriter.Tracker) {
				effectFnBuildReplacement(tracker, sf, result, "fnUntraced", "", result.PipeArguments, isFuncDecl)
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	// Fix 3: toEffectFnNoSpan - available when no-span variant is enabled
	if effectConfig.EffectFnIncludes(etscore.EffectFnNoSpan) {
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Effect.fn (no span)",
			Run: func(tracker *rewriter.Tracker) {
				effectFnBuildReplacement(tracker, sf, result, "fn", "", result.PipeArguments, isFuncDecl)
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	// Fix 4: toEffectFnSpanInferred - available when no explicit withSpan and inferred trace name exists
	if effectConfig.EffectFnIncludes(etscore.EffectFnInferredSpan) && result.ExplicitTraceExpression == nil && result.InferredTraceName != "" {
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Effect.fn(\"" + result.InferredTraceName + "\")",
			Run: func(tracker *rewriter.Tracker) {
				trace := strconv.Quote(result.InferredTraceName)
				effectFnBuildReplacement(tracker, sf, result, "fn", trace, result.PipeArguments, isFuncDecl)
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	// Fix 5: toEffectFnSpanSuggested - available when no explicit withSpan, has suggested trace name,
	// and (inferred-span is not enabled OR suggested name differs from inferred name)
	if effectConfig.EffectFnIncludes(etscore.EffectFnSuggestedSpan) && result.ExplicitTraceExpression == nil && result.SuggestedTraceName != "" &&
		(!effectConfig.EffectFnIncludes(etscore.EffectFnInferredSpan) || result.SuggestedTraceName != result.InferredTraceName) {
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Effect.fn(\"" + result.SuggestedTraceName + "\")",
			Run: func(tracker *rewriter.Tracker) {
				trace := strconv.Quote(result.SuggestedTraceName)
				effectFnBuildReplacement(tracker, sf, result, "fn", trace, result.PipeArguments, isFuncDecl)
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	return actions
}

// effectFnBuildReplacement edits around the original body rather than printing a
// cloned AST. The body and all surrounding declarations/properties retain their
// comments, literal spelling, whitespace, and semicolon style.
func effectFnBuildReplacement(
	tracker *rewriter.Tracker,
	sf *ast.SourceFile,
	result *typeparser.EffectFnOpportunityResult,
	variant string,
	trace string,
	pipeArgs []*ast.Node,
	isFuncDecl bool,
) {
	target := result.TargetNode
	body := typeparser.GetFunctionLikeBody(target)
	if result.HasGenBody && result.GeneratorFunction != nil {
		body = result.GeneratorFunction.Body
	}
	if body == nil {
		return
	}

	text := sf.Text()
	start := scanner.GetTokenPosOfNode(target, sf, false)
	bodyStart := scanner.GetTokenPosOfNode(body, sf, false)
	module := typeparser.FindEffectModuleIdentifier(sf)
	if result.EffectModule != nil {
		module = scanner.GetTextOfNode(result.EffectModule)
	}
	prefix := module + "." + variant
	if trace != "" {
		prefix += "(" + trace + ")"
	}
	prefix += "("
	var suffix strings.Builder

	if !result.HasGenBody && target.Kind == ast.KindFunctionExpression {
		// Keep the original function expression's signature verbatim.
		prefix += text[start:bodyStart]
	} else {
		prefix += "function"
		if result.HasGenBody {
			prefix += "*"
		}
		signature := effectFnSignatureRange(sf, target)
		signatureText := text[signature.Pos():signature.End()]
		if signatureText[0] != '(' && signatureText[0] != '<' {
			signatureText = "(" + signatureText + ")"
		}
		commentStart := start
		if isFuncDecl && target.Modifiers() != nil {
			commentStart = target.Modifiers().End()
		}
		prefix += effectFnComments(sf, commentStart, signature.Pos(), nil) + signatureText
		if comments := effectFnComments(sf, signature.End(), bodyStart, nil); comments != "" {
			prefix += comments
		} else {
			prefix += " "
		}
		if body.Kind != ast.KindBlock {
			prefix += "{ return "
			suffix.WriteString(" }")
		}
	}

	if isFuncDecl {
		decl := target.AsFunctionDeclaration()
		declarationPrefix := ""
		if modifiers := decl.Modifiers(); modifiers != nil {
			declarationPrefix = text[start:modifiers.End()] + " "
		}
		prefix = declarationPrefix + "const " + scanner.GetTextOfNode(decl.Name()) + " = " + prefix
	}

	cursor := body.End()
	for _, arg := range pipeArgs {
		// Include both leading and trailing trivia, including line comments before
		// the next comma/closing parenthesis. Never reprint the argument itself.
		end := scanner.SkipTrivia(text, arg.End())
		start := scanner.GetTokenPosOfNode(arg, sf, false)
		suffix.WriteString(",")
		suffix.WriteString(effectFnComments(sf, cursor, arg.Pos(), nil))
		suffix.WriteString(text[arg.Pos():start])
		suffix.WriteString(effectFnPipeArgumentText(arg, text[start:arg.End()]))
		suffix.WriteString(text[arg.End():end])
		cursor = end
	}
	var movedTrace *ast.Node
	if trace != "" {
		movedTrace = result.ExplicitTraceExpression
	}
	suffix.WriteString(effectFnComments(sf, cursor, target.End(), movedTrace))
	suffix.WriteString(")")
	if isFuncDecl {
		suffix.WriteString(";")
	}

	tracker.ReplaceTextRangeWithText(sf, core.NewTextRange(start, bodyStart), prefix)
	tracker.ReplaceTextRangeWithText(sf, core.NewTextRange(body.End(), target.End()), suffix.String())
}

// effectFnPipeArgumentText prevents Effect.fn's additional function arguments
// from reaching a bare pipeable's optional parameters. Factory calls already
// produce data-last pipeables and can be retained as written.
func effectFnPipeArgumentText(arg *ast.Node, text string) string {
	if ast.SkipParentheses(arg).Kind == ast.KindCallExpression {
		return text
	}

	// Avoid capturing references such as _.ignore in the new arrow's scope.
	identifiers := make(map[string]bool)
	var visit ast.Visitor
	visit = func(node *ast.Node) bool {
		if node.Kind == ast.KindIdentifier {
			identifiers[node.Text()] = true
		}
		node.ForEachChild(visit)
		return false
	}
	visit(arg)
	parameter := "_"
	for suffix := 1; identifiers[parameter]; suffix++ {
		parameter = "_" + strconv.Itoa(suffix)
	}

	switch arg.Kind {
	case ast.KindIdentifier, ast.KindPropertyAccessExpression, ast.KindElementAccessExpression, ast.KindParenthesizedExpression:
		// These expressions can be used directly as a callee.
	default:
		text = "(" + text + ")"
	}
	return parameter + " => " + text + "(" + parameter + ")"
}

// effectFnSignatureRange locates the complete parameter/type-parameter spelling,
// including comments and defaults. Bare arrow parameters need parentheses when
// moved to a function expression.
func effectFnSignatureRange(sf *ast.SourceFile, target *ast.Node) core.TextRange {
	text := sf.Text()
	params := typeparser.GetFunctionLikeParameters(target)
	start, end := params.Pos(), params.End()
	if start > 0 && text[start-1] == '(' {
		start--
		end = scanner.SkipTrivia(text, end) + 1 // closing parenthesis
	} else {
		return core.NewTextRange(scanner.SkipTrivia(text, start), end)
	}
	if typeParams := typeparser.GetFunctionLikeTypeParameters(target); typeParams != nil {
		start = typeParams.Pos() - 1 // opening angle bracket
	}
	return core.NewTextRange(start, end)
}

// effectFnComments retains comments from discarded wrapper syntax, such as the
// outer return statement. Arguments and the function body are copied separately.
func effectFnComments(sf *ast.SourceFile, start, end int, moved *ast.Node) string {
	text := sf.Text()[start:end]
	s := scanner.NewScanner()
	s.SetSkipTrivia(false)
	s.SetText(text)
	var comments strings.Builder
	triviaStart := 0
	hasComment := false
	for token := s.Scan(); token != ast.KindEndOfFile; token = s.Scan() {
		if moved != nil && start+s.TokenStart() >= scanner.GetTokenPosOfNode(moved, sf, false) && start+s.TokenStart() < moved.End() {
			if hasComment {
				comments.WriteString(text[triviaStart:s.TokenStart()])
			}
			triviaStart = min(moved.End(), end) - start
			s.ResetPos(triviaStart)
			hasComment = false
			continue
		}
		switch token {
		case ast.KindSingleLineCommentTrivia, ast.KindMultiLineCommentTrivia:
			comments.WriteString(text[triviaStart:s.TokenEnd()])
			triviaStart = s.TokenEnd()
			hasComment = true
		case ast.KindWhitespaceTrivia, ast.KindNewLineTrivia:
			// Whitespace is retained with the adjacent comment.
		default:
			if hasComment {
				comments.WriteString(text[triviaStart:s.TokenStart()])
			}
			// Use the parsed token's extent for contextual tokens (regexes and
			// template tails), so comment-like text inside literals is not moved.
			original := astnav.GetTokenAtPosition(sf, start+s.TokenStart())
			triviaStart = s.TokenEnd()
			if original.End() > start+triviaStart {
				triviaStart = min(original.End(), end) - start
				s.ResetPos(triviaStart)
			}
			hasComment = false
		}
	}
	if hasComment {
		comments.WriteString(text[triviaStart:])
	}
	return comments.String()
}
