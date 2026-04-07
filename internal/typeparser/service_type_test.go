package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

func TestServiceType_V4ServiceMapBeta43Shape(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}

	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, `
import { ServiceMap } from "effect"

export class UserRepo extends ServiceMap.Service<UserRepo, { readonly find: () => string }>()("UserRepo") {}
`)
	defer done()

	className, classType := findClassTypeByName(t, c, sf, "UserRepo")
	if classType == nil {
		t.Fatal("expected class type")
	}

	service := tp.ServiceType(classType, className)
	if service == nil {
		t.Fatal("expected v4 ServiceMap.Service type to parse")
	}
	if service.Identifier == nil {
		t.Fatal("expected service identifier type")
	}
	if service.Shape == nil {
		t.Fatal("expected service shape type")
	}
	if tp.ContextTag(classType, className) != nil {
		t.Fatal("expected v4 service not to parse as Context.Tag")
	}
}

func TestContextTag_V3LegacyShape(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect v3 not installed:", err)
	}

	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, `
import { Context } from "effect"

export class Config extends Context.Tag("Config")<Config, { readonly port: number }>() {}
`)
	defer done()

	className, classType := findClassTypeByName(t, c, sf, "Config")
	if classType == nil {
		t.Fatal("expected class type")
	}

	contextTag := tp.ContextTag(classType, className)
	if contextTag == nil {
		t.Fatal("expected v3 Context.Tag type to parse")
	}
	if contextTag.Identifier == nil {
		t.Fatal("expected context tag identifier type")
	}
	if contextTag.Shape == nil {
		t.Fatal("expected context tag shape type")
	}
	if tp.ServiceType(classType, className) != nil {
		t.Fatal("expected v3 Context.Tag not to parse as ServiceMap.Service")
	}
}

func findClassTypeByName(t *testing.T, c *checker.Checker, sf *ast.SourceFile, className string) (*ast.Node, *checker.Type) {
	t.Helper()

	var foundName *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil || foundName != nil {
			return false
		}
		if node.Kind == ast.KindClassDeclaration {
			classDecl := node.AsClassDeclaration()
			if classDecl != nil && node.Name() != nil && node.Name().Text() == className {
				foundName = node.Name()
				return false
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	if foundName == nil {
		t.Fatalf("class %q not found", className)
	}

	classSym := c.GetSymbolAtLocation(foundName)
	if classSym == nil {
		t.Fatalf("symbol for class %q not found", className)
	}
	classType := c.GetTypeOfSymbolAtLocation(classSym, foundName)
	if classType == nil {
		t.Fatalf("type for class %q not found", className)
	}
	return foundName, classType
}
