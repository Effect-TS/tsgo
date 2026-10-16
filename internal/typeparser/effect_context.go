package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

type EffectContextFlags uint8

const (
	EffectContextFlagNone           EffectContextFlags = 0
	EffectContextFlagCanYieldEffect EffectContextFlags = 1 << iota
	EffectContextFlagInEffectConstructorThunk
	EffectContextFlagPendingNextFunctionIsEffectThunk
	EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk
	EffectContextFlagInEffect = EffectContextFlagCanYieldEffect | EffectContextFlagInEffectConstructorThunk
)

// GetEffectContextFlags returns the Effect context of node. The analysis stores
// an entry only at the source file and at nodes whose value differs from their
// parent's, so a node answers with the entry of its closest ancestor.
func (tp *TypeParser) GetEffectContextFlags(node *ast.Node) EffectContextFlags {
	if tp == nil || tp.checker == nil || node == nil {
		return EffectContextFlagNone
	}
	flags := getClosestNodeLinks(&tp.links.EffectContextFlags, node)
	if flags == nil && tp.ensureEffectContextAnalyzed(node) {
		flags = getClosestNodeLinks(&tp.links.EffectContextFlags, node)
	}
	if flags == nil {
		return EffectContextFlagNone
	}
	return *flags & EffectContextFlagInEffect
}

// GetEffectYieldGeneratorFunction returns the Effect.gen or Effect.fn generator
// function node yields to, resolved like GetEffectContextFlags.
func (tp *TypeParser) GetEffectYieldGeneratorFunction(node *ast.Node) *ast.FunctionExpression {
	if tp == nil || tp.checker == nil || node == nil {
		return nil
	}
	genFn := getClosestNodeLinks(&tp.links.EffectYieldGeneratorFunction, node)
	if genFn == nil && tp.ensureEffectContextAnalyzed(node) {
		genFn = getClosestNodeLinks(&tp.links.EffectYieldGeneratorFunction, node)
	}
	if genFn == nil {
		return nil
	}
	return *genFn
}

// getClosestNodeLinks returns the entry of node or of its closest ancestor
// with one. The source file always has an entry once analyzed, so nil means
// the file has not been analyzed yet.
func getClosestNodeLinks[T any](store *checker.NodeLinkStore[T], node *ast.Node) *T {
	for current := node; current != nil; current = current.Parent {
		if links := store.TryGet(current); links != nil {
			return links
		}
	}
	return nil
}

// ensureEffectContextAnalyzed analyzes the source file of node once and reports
// whether it has been analyzed.
func (tp *TypeParser) ensureEffectContextAnalyzed(node *ast.Node) bool {
	sf := ast.GetSourceFileOfNode(node)
	if sf == nil {
		return false
	}
	return Cached(&tp.links.EffectContextAnalyzed, sf, func() bool {
		tp.analyzeEffectContextForSourceFile(sf)
		return true
	})
}

// effectContextRegistration is state a node registers for one of its
// descendants, applied when the walk reaches that descendant.
type effectContextRegistration struct {
	node              *ast.Node
	enableFlags       EffectContextFlags
	disableFlags      EffectContextFlags
	generatorFunction *ast.FunctionExpression
	setsGenerator     bool
}

func (tp *TypeParser) analyzeEffectContextForSourceFile(sf *ast.SourceFile) {
	if tp == nil || tp.checker == nil || sf == nil {
		return
	}
	links := tp.links

	var walk ast.Visitor
	// registrations of the ancestors of the visited node, truncated after each subtree
	var registrations []effectContextRegistration
	// state of the parent of the visited node, and the flags it disables on its children
	var parentFlags EffectContextFlags
	var parentGeneratorFunction *ast.FunctionExpression
	var parentChildrenDisableFlags EffectContextFlags
	pendingFlagsMask := EffectContextFlagPendingNextFunctionIsEffectThunk | EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk
	functionScopeResetFlags := EffectContextFlagCanYieldEffect | EffectContextFlagInEffectConstructorThunk | pendingFlagsMask

	setPendingEnableFlags := func(node *ast.Node, flags EffectContextFlags) {
		if node == nil || flags == EffectContextFlagNone {
			return
		}
		registrations = append(registrations, effectContextRegistration{node: node, enableFlags: flags})
	}

	setPendingDisableFlags := func(node *ast.Node, flags EffectContextFlags) {
		if node == nil || flags == EffectContextFlagNone {
			return
		}
		registrations = append(registrations, effectContextRegistration{node: node, disableFlags: flags})
	}

	setPendingGeneratorFunction := func(node *ast.Node, genFn *ast.FunctionExpression) {
		registrations = append(registrations, effectContextRegistration{node: node, generatorFunction: genFn, setsGenerator: true})
	}

	transparentPendingExpression := func(node *ast.Node) *ast.Node {
		if node == nil {
			return nil
		}
		switch node.Kind {
		case ast.KindParenthesizedExpression, ast.KindSatisfiesExpression, ast.KindAsExpression, ast.KindNonNullExpression, ast.KindTypeAssertionExpression:
			return node.Expression()
		default:
			return nil
		}
	}

	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		registrationsLen := len(registrations)

		// inherit from parent, if any; the source file starts with no flags.
		flags := parentFlags
		generatorFunction := parentGeneratorFunction
		disableFlags := parentChildrenDisableFlags
		var enableFlags EffectContextFlags
		for i := range registrations {
			registration := &registrations[i]
			if registration.node != node {
				continue
			}
			disableFlags |= registration.disableFlags
			enableFlags |= registration.enableFlags
			if registration.setsGenerator {
				generatorFunction = registration.generatorFunction
			}
		}

		// disable pending disable flags, then merge pending state for this node
		flags = flags&^disableFlags | enableFlags

		var childrenDisableFlags EffectContextFlags
		if flags&EffectContextFlagPendingNextFunctionIsEffectThunk != 0 && (node.Kind == ast.KindArrowFunction || node.Kind == ast.KindFunctionExpression) {
			if body := node.Body(); body != nil {
				setPendingEnableFlags(body.AsNode(), EffectContextFlagInEffectConstructorThunk)
				setPendingDisableFlags(body.AsNode(), pendingFlagsMask)
			}
		} else if flags&EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk != 0 && node.Kind == ast.KindObjectLiteralExpression {
			childrenDisableFlags |= pendingFlagsMask
			obj := node.AsObjectLiteralExpression()
			if obj != nil && obj.Properties != nil {
				for _, prop := range obj.Properties.Nodes {
					if prop == nil || prop.Kind != ast.KindPropertyAssignment {
						continue
					}
					assignment := prop.AsPropertyAssignment()
					if assignment == nil || assignment.Name() == nil || assignment.Initializer == nil {
						continue
					}
					if assignment.Name().Text() != "try" {
						continue
					}
					setPendingEnableFlags(assignment.Initializer, EffectContextFlagPendingNextFunctionIsEffectThunk)
				}
			}
		} else if expr := transparentPendingExpression(node); expr != nil {
			setPendingEnableFlags(expr, flags&pendingFlagsMask)
		} else if flags&pendingFlagsMask != 0 {
			childrenDisableFlags |= pendingFlagsMask
		}

		// logic for this node
		if effectGen := tp.EffectGenCall(node); effectGen != nil {
			bodyNode := effectGen.body.AsNode()
			setPendingEnableFlags(bodyNode, EffectContextFlagCanYieldEffect)
			setPendingGeneratorFunction(bodyNode, effectGen.generatorFunction)
		} else if effectFn := tp.EffectFnCall(node); effectFn != nil && effectFn.IsGenerator() {
			body := effectFn.Body()
			genFn := effectFn.GeneratorFunction()
			if body != nil && genFn != nil {
				bodyNode := body.AsNode()
				setPendingEnableFlags(bodyNode, EffectContextFlagCanYieldEffect)
				setPendingGeneratorFunction(bodyNode, genFn)
			}
		}

		if node.Kind == ast.KindCallExpression {
			call := node.AsCallExpression()
			if call != nil && call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
				effectThunkArg := call.Arguments.Nodes[0]
				switch {
				case tp.IsNodeReferenceToEffectModuleApi(call.Expression, "sync"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "promise"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "callback"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "suspend"):
					setPendingEnableFlags(effectThunkArg, EffectContextFlagPendingNextFunctionIsEffectThunk)
				case tp.IsNodeReferenceToEffectModuleApi(call.Expression, "try"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "tryPromise"):
					setPendingEnableFlags(effectThunkArg, EffectContextFlagPendingNextFunctionIsEffectThunk|EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk)
				}
			}
		}

		// Function-like nodes create a new scope, so they should not directly inherit
		// yieldability from an outer Effect scope. Matching Effect helpers re-enable the
		// flag on the specific body node below.
		if ast.IsFunctionLike(node) {
			childrenDisableFlags |= functionScopeResetFlags
		}

		// reset stores correlated to a flag set here.
		if flags&EffectContextFlagCanYieldEffect == 0 {
			generatorFunction = nil
		}

		// store only what differs from the parent, reads resolve to the closest node with an entry.
		isSourceFile := node.Parent == nil
		if isSourceFile || flags&EffectContextFlagInEffect != parentFlags&EffectContextFlagInEffect {
			*links.EffectContextFlags.Get(node) = flags
		}
		if isSourceFile || generatorFunction != parentGeneratorFunction {
			*links.EffectYieldGeneratorFunction.Get(node) = generatorFunction
		}

		savedFlags, savedGeneratorFunction, savedDisableFlags := parentFlags, parentGeneratorFunction, parentChildrenDisableFlags
		parentFlags, parentGeneratorFunction, parentChildrenDisableFlags = flags, generatorFunction, childrenDisableFlags
		node.ForEachChild(walk)
		parentFlags, parentGeneratorFunction, parentChildrenDisableFlags = savedFlags, savedGeneratorFunction, savedDisableFlags
		registrations = registrations[:registrationsLen]
		return false
	}
	walk(sf.AsNode())
}
