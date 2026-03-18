package refactors

import (
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/effect-ts/effect-typescript-go/internal/schemagen"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var TypeToEffectSchema = refactor.Refactor{
	Name:        "typeToEffectSchema",
	Description: "Generate Effect.Schema from type",
	Kind:        "rewrite.effect.typeToEffectSchema",
	Run:         runTypeToEffectSchema,
}

// findInterfaceOrTypeAlias walks the ancestor chain from the token at the cursor
// to find an InterfaceDeclaration or TypeAliasDeclaration whose name overlaps the
// selection span, and that has no type parameters.
func findInterfaceOrTypeAlias(ctx *refactor.Context) *ast.Node {
	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	for node := token; node != nil && node.Kind != ast.KindSourceFile; node = node.Parent {
		switch node.Kind {
		case ast.KindInterfaceDeclaration:
			iface := node.AsInterfaceDeclaration()
			// Must have no type parameters
			if iface.TypeParameters != nil && len(iface.TypeParameters.Nodes) > 0 {
				continue
			}
			// Name must overlap the selection
			name := iface.Name()
			namePos := astnav.GetStartOfNode(name, ctx.SourceFile, false)
			nameEnd := name.End()
			if ctx.Span.Pos() < namePos || ctx.Span.Pos() >= nameEnd {
				continue
			}
			return node
		case ast.KindTypeAliasDeclaration:
			ta := node.AsTypeAliasDeclaration()
			// Must have no type parameters
			if ta.TypeParameters != nil && len(ta.TypeParameters.Nodes) > 0 {
				continue
			}
			// Name must overlap the selection
			name := ta.Name()
			namePos := astnav.GetStartOfNode(name, ctx.SourceFile, false)
			nameEnd := name.End()
			if ctx.Span.Pos() < namePos || ctx.Span.Pos() >= nameEnd {
				continue
			}
			return node
		}
	}

	return nil
}

func runTypeToEffectSchema(ctx *refactor.Context) []ls.CodeAction {
	matchedNode := findInterfaceOrTypeAlias(ctx)
	if matchedNode == nil {
		return nil
	}

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	version := typeparser.SupportedEffectVersion(c)

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Generate Effect.Schema from type",
		Run: func(tracker *change.Tracker) {
			gen := schemagen.New(tracker, ctx.SourceFile, version)
			newNode := gen.Process(matchedNode, false)
			if newNode != nil {
				tracker.InsertNodeBefore(ctx.SourceFile, matchedNode, newNode, true, change.LeadingTriviaOptionNone)
			}
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.typeToEffectSchema"
	return []ls.CodeAction{*action}
}
