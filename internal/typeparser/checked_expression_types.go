package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// RecordCheckedExpressionType stores the type the checker computed for an
// expression while checking a source file. Only final evaluations count: plain
// Normal mode, or Contextual alone when a call argument is checked against its
// parameter. Inference passes and type-only queries are tentative. The checker
// may evaluate a node more than once, and the last final evaluation is the one
// its diagnostics are based on, so later writes win. Rule runs pause recording,
// and evaluations outside a source-file check are ignored, so only the check
// itself writes the store.
func RecordCheckedExpressionType(c *checker.Checker, node *ast.Node, checkMode checker.CheckMode, t *checker.Type) {
	if checkMode&^checker.CheckModeContextual != 0 || !checker.IsCheckingSourceFile(c) {
		return
	}
	links := effectLinksOf(c)
	if links.checkedExpressionTypesPaused {
		return
	}
	*links.CheckedExpressionType.Get(node) = t
}

// PauseCheckedExpressionTypes stops recording for the checker until the
// returned function is called.
func PauseCheckedExpressionTypes(c *checker.Checker) func() {
	links := effectLinksOf(c)
	links.checkedExpressionTypesPaused = true
	return func() { links.checkedExpressionTypesPaused = false }
}

// checkedExpressionType returns the type the file's check computed for an
// expression, without evaluating anything, or nil when the check never
// evaluated it.
func (tp TypeParser) checkedExpressionType(node *ast.Node) *checker.Type {
	if !ast.IsExpressionNode(node) || ast.IsPartOfTypeNode(node) {
		return nil
	}
	if ast.IsRightSideOfQualifiedNameOrPropertyAccess(node) {
		node = node.Parent
	}
	// A checked file stays checked, so remember the last one found checked. An
	// unchecked file can be checked later and is asked again.
	sf := ast.GetSourceFileOfNode(node)
	if sf == nil {
		return nil
	}
	if sf != tp.links.checkedExpressionTypeSourceFile {
		if !checker.IsSourceFileTypeChecked(tp.checker, sf) {
			return nil
		}
		tp.links.checkedExpressionTypeSourceFile = sf
	}
	var t *checker.Type
	if recorded := tp.links.CheckedExpressionType.TryGet(node); recorded != nil {
		t = *recorded
	}
	if t == nil {
		// Initializers and return expressions first evaluated while recording
		// was paused are only in the checker's own cache.
		t = checker.GetResolvedTypeFromTypeNode(tp.checker, node)
	}
	if t == nil {
		return nil
	}
	return checker.Checker_getRegularTypeOfLiteralType(tp.checker, t)
}
