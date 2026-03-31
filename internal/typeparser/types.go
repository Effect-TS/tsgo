// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// EffectTypeId is the property key for Effect's variance struct.
// Effect v4 (effect-smol) uses this pattern to encode type parameters.
const EffectTypeId = "~effect/Effect"

// LayerTypeId is the property key for Layer's variance struct.
const LayerTypeId = "~effect/Layer"

// ServiceTypeId is the property key for Service's variance struct.
const ServiceTypeId = "~effect/ServiceMap/Service"

// SchemaTypeId is the property key for Schema's variance struct.
const SchemaTypeId = "~effect/Schema/Schema"

// ScopeTypeId is the property key for Scope's variance struct.
const ScopeTypeId = "~effect/Scope"

// Effect represents parsed Effect<A, E, R> type parameters.
type Effect struct {
	A *checker.Type // Success type
	E *checker.Type // Error type
	R *checker.Type // Requirements type
}

// Layer represents parsed Layer<ROut, E, RIn> type parameters.
type Layer struct {
	ROut *checker.Type // Provided services (contravariant)
	E    *checker.Type // Error type (covariant)
	RIn  *checker.Type // Required services (covariant)
}

// Service represents parsed ServiceMap.Service<Identifier, Shape> type parameters.
type Service struct {
	Identifier *checker.Type // The service identifier/tag type
	Shape      *checker.Type // The service implementation shape
}

// EffectGenCallResult represents a parsed Effect.gen(...) call.
type EffectGenCallResult struct {
	Call              *ast.CallExpression
	EffectModule      *ast.Expression
	GeneratorFunction *ast.FunctionExpression
	Body              *ast.BlockOrExpression
}

type EffectFnVariant string

const (
	EffectFnVariantFn              EffectFnVariant = "fn"
	EffectFnVariantFnUntraced      EffectFnVariant = "fnUntraced"
	EffectFnVariantFnUntracedEager EffectFnVariant = "fnUntracedEager"
)

// TransformationKind represents how a transformation was expressed in source code.
type TransformationKind string

const (
	TransformationKindPipe             TransformationKind = "pipe"
	TransformationKindPipeable         TransformationKind = "pipeable"
	TransformationKindCall             TransformationKind = "call"
	TransformationKindEffectFn         TransformationKind = "effectFn"
	TransformationKindEffectFnUntraced TransformationKind = "effectFnUntraced"
)

// PipingFlowTransformation represents a single transformation step in a piping flow.
type PipingFlowTransformation struct {
	Kind    TransformationKind // How the transformation was expressed
	Node    *ast.Node          // The full transformation node (call expression or bare callee)
	Callee  *ast.Node          // The function being applied (e.g., Effect.map)
	Args    []*ast.Node        // Arguments to the transformation, or nil for constants/single-arg calls
	OutType *checker.Type      // The resulting type after this transformation (may be nil)
}

// PipingFlowSubject is the starting expression of a piping flow.
type PipingFlowSubject struct {
	Node    *ast.Node     // The expression node
	OutType *checker.Type // The type of the subject expression (may be nil)
}

// PipingFlow represents a complete piping flow: a subject followed by transformations.
type PipingFlow struct {
	Node            *ast.Node                  // The outermost expression encompassing the entire flow
	Subject         PipingFlowSubject          // The starting expression and its type
	Transformations []PipingFlowTransformation // Ordered list of transformations
}

// EffectFnCallResult represents a parsed Effect.fn-family call.
type EffectFnCallResult struct {
	Call            *ast.CallExpression
	Variant         EffectFnVariant
	EffectModule    *ast.Expression
	FunctionNode    *ast.Node   // ArrowFunction or FunctionExpression
	PipeArguments   []*ast.Node // Transformation args after the body (may be empty/nil)
	TraceExpression *ast.Node   // The name string from curried Effect.fn("name")(...), or nil
}

func (r *EffectFnCallResult) IsGenerator() bool {
	return r.GeneratorFunction() != nil
}

func (r *EffectFnCallResult) GeneratorFunction() *ast.FunctionExpression {
	if r == nil || r.FunctionNode == nil || r.FunctionNode.Kind != ast.KindFunctionExpression {
		return nil
	}
	fn := r.FunctionNode.AsFunctionExpression()
	if fn == nil || fn.AsteriskToken == nil {
		return nil
	}
	return fn
}

func (r *EffectFnCallResult) Body() *ast.BlockOrExpression {
	if r == nil || r.FunctionNode == nil {
		return nil
	}
	switch r.FunctionNode.Kind {
	case ast.KindArrowFunction:
		fn := r.FunctionNode.AsArrowFunction()
		if fn == nil {
			return nil
		}
		return fn.Body
	case ast.KindFunctionExpression:
		fn := r.FunctionNode.AsFunctionExpression()
		if fn == nil {
			return nil
		}
		return fn.Body
	default:
		return nil
	}
}

// EffectFnIifeResult represents a parsed Effect.fn(...)() or Effect.fnUntraced(...)() IIFE.
type EffectFnIifeResult struct {
	OuterCall         *ast.CallExpression     // The entire IIFE expression (the outer () call)
	InnerCall         *ast.CallExpression     // The Effect.fn(...) call
	EffectModule      *ast.Expression         // The Effect module identifier
	Variant           string                  // "fn", "fnUntraced", or "fnUntracedEager"
	GeneratorFunction *ast.FunctionExpression // Non-nil for generator bodies (fix available)
	PipeArguments     []*ast.Node             // Trailing transformation args (may be nil)
	TraceExpression   *ast.Node               // The name string from curried form, or nil
}

// ParsedLazyExpression represents a parsed arrow function or function expression
// with its inner expression extracted.
type ParsedLazyExpression struct {
	Node       *ast.Node   // The original ArrowFunction or FunctionExpression node
	Params     []*ast.Node // Parameter declarations (empty when parsed with thunk=true)
	Body       *ast.Node   // The function body as written (Expression or Block)
	Expression *ast.Node   // The inner expression (return value)
}

// ExpectedAndRealType represents a pair of expected and actual types at an assignment site.
// This is used by diagnostic rules that need to compare what type was expected at a location
// versus what type was actually provided.
type ExpectedAndRealType struct {
	Node         *ast.Node     // The location node (for diagnostic reporting)
	ExpectedType *checker.Type // The type expected at this location
	ValueNode    *ast.Node     // The actual value node
	RealType     *checker.Type // The actual type of the value
}

// EffectFnOpportunityResult represents a function that can be converted to Effect.fn.
type EffectFnOpportunityResult struct {
	TargetNode              *ast.Node               // The function node being reported
	NameIdentifier          *ast.Node               // The discovered name node for the function
	GeneratorFunction       *ast.FunctionExpression // Non-nil for gen opportunity, nil for regular
	PipeArguments           []*ast.Node             // Pipe args from piped Effect.gen (may be empty)
	ExplicitTraceExpression *ast.Node               // Span name from Effect.withSpan if last pipe arg, or nil
	SuggestedTraceName      string                  // The local function/variable name for suggested span
	InferredTraceName       string                  // Context-aware name (e.g., "ServiceTag.member" or exported name)
	EffectModule            *ast.Expression         // The Effect module identifier
	HasGenBody              bool                    // True for gen opportunity, false for regular
	IsLayerMember           bool                    // True when the target is a property value inside a Layer service definition
}

// UnrollUnionMembers returns the constituent types of a union type,
// or a single-element slice containing the type itself if it's not a union.
func UnrollUnionMembers(t *checker.Type) []*checker.Type {
	if t == nil {
		return nil
	}
	if t.Flags()&checker.TypeFlagsUnion != 0 {
		return t.Types()
	}
	return []*checker.Type{t}
}

// UnrollIntersectionMembers returns the constituent types of an intersection type,
// or a single-element slice containing the type itself if it's not an intersection.
func UnrollIntersectionMembers(t *checker.Type) []*checker.Type {
	if t == nil {
		return nil
	}
	if t.Flags()&checker.TypeFlagsIntersection != 0 {
		return t.Types()
	}
	return []*checker.Type{t}
}
