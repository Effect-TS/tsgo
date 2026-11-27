package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectSqlStatementModuleDescriptor = newPackageSourceFileDescriptor(moduleDescriptorSqlStatement, "effect", isEffectSqlStatementSourceFile)

func isEffectSqlStatementSourceFile(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
	if moduleSym == nil {
		return false
	}

	for _, name := range []string{"statement", "isFragment", "makeCompiler", "defaultEscape"} {
		if c.TryGetMemberInModuleExportsAndProperties(name, moduleSym) == nil {
			return false
		}
	}
	return true
}

type sqlConstructorCheck uint8

const (
	sqlConstructorCheckInProgress sqlConstructorCheck = iota
	sqlConstructorCheckMatched
	sqlConstructorCheckUnmatched
)

type sqlConstructorChecks map[*checker.Type]sqlConstructorCheck

func (tp *TypeParser) IsEffectSqlStatementConstructorTaggedTemplate(node *ast.Node) bool {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindTaggedTemplateExpression {
		return false
	}
	tagged := node.AsTaggedTemplateExpression()
	if tagged == nil || tagged.Tag == nil {
		return false
	}
	return tp.extendsEffectSqlStatementConstructor(tp.GetTypeAtLocation(tagged.Tag), sqlConstructorChecks{})
}

func (tp *TypeParser) extendsEffectSqlStatementConstructor(t *checker.Type, checks sqlConstructorChecks) bool {
	if t == nil {
		return false
	}
	t = tp.checker.GetApparentType(t)
	if t.Flags()&checker.TypeFlagsUnion != 0 {
		for _, member := range t.Types() {
			if !tp.extendsEffectSqlStatementConstructor(member, checks) {
				return false
			}
		}
		return len(t.Types()) > 0
	}
	if t.Flags()&checker.TypeFlagsIntersection != 0 {
		for _, member := range t.Types() {
			if tp.extendsEffectSqlStatementConstructor(member, checks) {
				return true
			}
		}
		return false
	}
	if t.Flags()&checker.TypeFlagsObject == 0 {
		return false
	}
	if t.ObjectFlags()&checker.ObjectFlagsReference != 0 && t.Target() != nil {
		t = t.Target()
	}
	if t.ObjectFlags()&checker.ObjectFlagsClassOrInterface == 0 {
		return false
	}
	if check, visited := checks[t]; visited {
		return check == sqlConstructorCheckMatched
	}
	checks[t] = sqlConstructorCheckInProgress
	if tp.interfaceExtendsEffectSqlStatementConstructor(t, checks) {
		checks[t] = sqlConstructorCheckMatched
		return true
	}
	checks[t] = sqlConstructorCheckUnmatched
	return false
}

func (tp *TypeParser) interfaceExtendsEffectSqlStatementConstructor(t *checker.Type, checks sqlConstructorChecks) bool {
	if tp.isEffectSqlStatementConstructorInterface(t) {
		return true
	}
	for _, base := range tp.checker.GetBaseTypes(t) {
		if tp.extendsEffectSqlStatementConstructor(base, checks) {
			return true
		}
	}
	return false
}

func (tp *TypeParser) isEffectSqlStatementConstructorInterface(t *checker.Type) bool {
	sym := t.Symbol()
	if sym == nil {
		return false
	}
	for _, decl := range ast.TSGetSymbolDeclarations(sym) {
		if decl == nil || decl.Kind != ast.KindInterfaceDeclaration || decl.Name() == nil {
			continue
		}
		if tp.IsNodeReferenceToModuleExport(decl.Name(), effectSqlStatementModuleDescriptor, "Constructor") {
			return true
		}
	}
	return false
}
