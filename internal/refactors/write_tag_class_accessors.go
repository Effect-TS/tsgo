package refactors

import (
	"strings"

	"github.com/effect-ts/effect-typescript-go/internal/effectutil"
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var WriteTagClassAccessors = refactor.Refactor{
	Name:        "writeTagClassAccessors",
	Description: "Implement Service accessors",
	Kind:        "rewrite.effect.writeTagClassAccessors",
	Run:         runWriteTagClassAccessors,
}

// involvedMember holds a property symbol and its type for accessor generation.
type involvedMember struct {
	property     *ast.Symbol
	propertyType *checker.Type
}

func runWriteTagClassAccessors(ctx *refactor.Context) []ls.CodeAction {
	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	// V3-only refactor
	if typeparser.SupportedEffectVersion(c) == typeparser.EffectMajorV4 {
		return nil
	}

	// Walk ancestors to find a ClassDeclaration
	var classNode *ast.Node
	for node := token; node != nil && node.Kind != ast.KindSourceFile; node = node.Parent {
		if node.Kind == ast.KindClassDeclaration {
			classNode = node
			break
		}
	}
	if classNode == nil {
		return nil
	}

	// Check ExtendsEffectService (with accessors: true) or ExtendsEffectTag
	var className *ast.Node
	serviceResult := typeparser.ExtendsEffectService(c, classNode)
	if serviceResult != nil {
		if !hasAccessorsTrue(serviceResult.Options) {
			return nil
		}
		className = serviceResult.ClassName
	} else {
		tagResult := typeparser.ExtendsEffectTag(c, classNode)
		if tagResult != nil {
			// Effect.Tag always has accessors
			className = tagResult.ClassName
		} else {
			return nil
		}
	}

	if className == nil {
		return nil
	}

	// Resolve the service shape type via the _Service variance struct
	classSymbol := c.GetSymbolAtLocation(className)
	if classSymbol == nil {
		return nil
	}
	classType := c.GetTypeOfSymbolAtLocation(classSymbol, classNode)
	if classType == nil {
		return nil
	}

	contextTag := typeparser.ContextTag(c, classType, classNode)
	if contextTag == nil || contextTag.Shape == nil {
		return nil
	}

	// Unroll union and filter out primitives
	nonPrimitiveServices := filterNonPrimitiveTypes(typeparser.UnrollUnionMembers(contextTag.Shape))
	if len(nonPrimitiveServices) == 0 {
		return nil
	}

	// Find involved members (those with generics or multiple overloads)
	var members []involvedMember
	seen := make(map[string]bool)
	for _, service := range nonPrimitiveServices {
		for _, property := range c.GetPropertiesOfType(service) {
			propName := property.Name
			if seen[propName] {
				continue
			}
			propertyType := c.GetTypeOfSymbolAtLocation(property, classNode)
			callSignatures := c.GetSignaturesOfType(propertyType, checker.SignatureKindCall)
			if len(callSignatures) == 0 {
				continue
			}
			hasGenerics := false
			for _, sig := range callSignatures {
				if len(sig.TypeParameters()) > 0 {
					hasGenerics = true
					break
				}
			}
			if len(callSignatures) > 1 || hasGenerics {
				members = append(members, involvedMember{property: property, propertyType: propertyType})
				seen[propName] = true
			}
		}
	}

	if len(members) == 0 {
		return nil
	}

	effectIdentifier := effectutil.FindEffectModuleIdentifier(ctx.SourceFile)
	classNameText := scanner.GetTextOfNode(className)

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Implement Service accessors",
		Run: func(tracker *change.Tracker) {
			generateAccessors(tracker, ctx, c, classNode, classNameText, effectIdentifier, members)
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.writeTagClassAccessors"
	return []ls.CodeAction{*action}
}

// hasAccessorsTrue checks if an options object literal has `accessors: true`.
func hasAccessorsTrue(options *ast.Node) bool {
	if options == nil || options.Kind != ast.KindObjectLiteralExpression {
		return false
	}
	objLit := options.AsObjectLiteralExpression()
	if objLit == nil || objLit.Properties == nil {
		return false
	}
	for _, prop := range objLit.Properties.Nodes {
		if prop == nil || prop.Kind != ast.KindPropertyAssignment {
			continue
		}
		pa := prop.AsPropertyAssignment()
		if pa == nil || pa.Name() == nil {
			continue
		}
		name := scanner.GetTextOfNode(pa.Name())
		if name == "accessors" && pa.Initializer != nil && pa.Initializer.Kind == ast.KindTrueKeyword {
			return true
		}
	}
	return false
}

// filterNonPrimitiveTypes filters out primitive types (number, string, boolean, literals).
func filterNonPrimitiveTypes(types []*checker.Type) []*checker.Type {
	var result []*checker.Type
	for _, t := range types {
		flags := t.Flags()
		if flags&checker.TypeFlagsNumber != 0 ||
			flags&checker.TypeFlagsString != 0 ||
			flags&checker.TypeFlagsBoolean != 0 ||
			flags&checker.TypeFlagsLiteral != 0 {
			continue
		}
		result = append(result, t)
	}
	return result
}

// generateAccessors generates accessor text directly via InsertText.
func generateAccessors(
	tracker *change.Tracker,
	ctx *refactor.Context,
	c *checker.Checker,
	classNode *ast.Node,
	classNameText string,
	effectIdentifier string,
	members []involvedMember,
) {
	classDecl := classNode.AsClassDeclaration()
	if classDecl == nil {
		return
	}

	// Find insert location: first member's pos, or end of class - 1
	insertPos := core.TextPos(classNode.End() - 1)
	if classDecl.Members != nil && len(classDecl.Members.Nodes) > 0 {
		insertPos = core.TextPos(classDecl.Members.Nodes[0].Pos())
	}
	lsPos := ctx.BytePosToLSPPosition(int(insertPos))

	var sb strings.Builder
	for _, member := range members {
		propertyName := member.property.Name
		callSignatures := c.GetSignaturesOfType(member.propertyType, checker.SignatureKindCall)
		if len(callSignatures) == 0 {
			continue
		}

		forceAny := len(callSignatures) > 1

		// Build signature type strings
		var sigTypeStrs []string
		for _, sig := range callSignatures {
			sigStr := buildProxySignatureText(c, sig, classNode, classNameText, effectIdentifier)
			if sigStr != "" {
				sigTypeStrs = append(sigTypeStrs, sigStr)
			}
		}
		if len(sigTypeStrs) == 0 {
			continue
		}

		// Build type annotation
		var typeAnnotation string
		if len(sigTypeStrs) == 1 {
			typeAnnotation = sigTypeStrs[0]
		} else {
			typeAnnotation = "(" + strings.Join(sigTypeStrs, ") & (") + ")"
		}

		// Build the property text
		sb.WriteString("\n  static override ")
		sb.WriteString(propertyName)
		sb.WriteString(": ")
		sb.WriteString(typeAnnotation)
		sb.WriteString(" = (")
		if forceAny {
			sb.WriteString("...args: any[]")
		} else {
			sb.WriteString("...args")
		}
		sb.WriteString(") => ")
		sb.WriteString(effectIdentifier)
		sb.WriteString(".andThen(")
		sb.WriteString(classNameText)
		sb.WriteString(", (")
		if forceAny {
			sb.WriteString("_: any")
		} else {
			sb.WriteString("_")
		}
		sb.WriteString(") => _.")
		sb.WriteString(propertyName)
		sb.WriteString("(...args))")
		if forceAny {
			sb.WriteString(" as any")
		}
	}

	if sb.Len() > 0 {
		tracker.InsertText(ctx.SourceFile, lsPos, sb.String()+"\n")
	}
}

// buildProxySignatureText builds the text representation of a proxy function type for a call signature.
func buildProxySignatureText(
	c *checker.Checker,
	sig *checker.Signature,
	classNode *ast.Node,
	classNameText string,
	effectIdentifier string,
) string {
	var sb strings.Builder

	// Type parameters
	if tps := sig.TypeParameters(); len(tps) > 0 {
		sb.WriteString("<")
		for i, tp := range tps {
			if i > 0 {
				sb.WriteString(", ")
			}
			sb.WriteString(tp.Symbol().Name)
			constraintType := c.GetConstraintOfTypeParameter(tp)
			if constraintType != nil {
				sb.WriteString(" extends ")
				sb.WriteString(c.TypeToStringEx(constraintType, classNode, checker.TypeFormatFlagsNoTruncation))
			}
		}
		sb.WriteString(">")
	}

	// Parameters
	sb.WriteString("(")
	params := sig.Parameters()
	for i, param := range params {
		if i > 0 {
			sb.WriteString(", ")
		}
		if sig.HasRestParameter() && i == len(params)-1 {
			sb.WriteString("...")
		}
		sb.WriteString(param.Name)
		sb.WriteString(": ")
		paramType := c.GetTypeOfSymbolAtLocation(param, classNode)
		sb.WriteString(c.TypeToStringEx(paramType, classNode, checker.TypeFormatFlagsNoTruncation))
	}
	sb.WriteString(")")

	// Return type
	sb.WriteString(" => ")
	returnType := c.GetReturnTypeOfSignature(sig)
	wrappedReturn := buildWrappedReturnTypeText(c, returnType, classNode, classNameText, effectIdentifier)
	sb.WriteString(wrappedReturn)

	return sb.String()
}

// buildWrappedReturnTypeText builds the text representation of the wrapped return type.
func buildWrappedReturnTypeText(
	c *checker.Checker,
	returnType *checker.Type,
	classNode *ast.Node,
	classNameText string,
	effectIdentifier string,
) string {
	// Try to parse as Effect type
	effect := typeparser.EffectType(c, returnType, classNode)
	if effect != nil {
		aStr := c.TypeToStringEx(effect.A, classNode, checker.TypeFormatFlagsNoTruncation)
		eStr := c.TypeToStringEx(effect.E, classNode, checker.TypeFormatFlagsNoTruncation)

		var rStr string
		if effect.R != nil && effect.R.Flags()&checker.TypeFlagsNever != 0 {
			rStr = classNameText
		} else {
			rStr = classNameText + " | " + c.TypeToStringEx(effect.R, classNode, checker.TypeFormatFlagsNoTruncation)
		}

		return effectIdentifier + ".Effect<" + aStr + ", " + eStr + ", " + rStr + ">"
	}

	// Try to detect Promise<T>
	returnTypeStr := c.TypeToStringEx(returnType, classNode, checker.TypeFormatFlagsNoTruncation)
	if innerTypeStr, ok := isPromiseTypeString(returnTypeStr); ok {
		return effectIdentifier + ".Effect<" + innerTypeStr + ", Cause.UnknownException, " + classNameText + ">"
	}

	// Fallback: Effect<A, never, ClassName>
	aStr := c.TypeToStringEx(returnType, classNode, checker.TypeFormatFlagsNoTruncation)
	return effectIdentifier + ".Effect<" + aStr + ", never, " + classNameText + ">"
}

// isPromiseTypeString checks if the type string represents a Promise<T> and extracts the inner type string.
func isPromiseTypeString(typeStr string) (string, bool) {
	if !strings.HasPrefix(typeStr, "Promise<") {
		return "", false
	}
	depth := 0
	for i := 8; i < len(typeStr); i++ {
		switch typeStr[i] {
		case '<':
			depth++
		case '>':
			if depth == 0 {
				return typeStr[8:i], true
			}
			depth--
		}
	}
	return "", false
}
