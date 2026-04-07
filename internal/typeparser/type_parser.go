package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/packagejson"
)

// TypeParser groups checker-backed typeparser operations behind a shared
// checker/program pair so callers do not need to thread them through each call.
type TypeParser struct {
	program checker.Program
	checker *checker.Checker
	links   *EffectLinks
}

// EffectLinks holds per-checker cached type-parser results.
// One instance is lazily created per Checker and cached on TypeParser.
type EffectLinks struct {
	TypeAtLocation       core.LinkStore[*ast.Node, *checker.Type]
	EffectType           core.LinkStore[*checker.Type, *Effect]
	StreamType           core.LinkStore[*checker.Type, *Effect]
	StrictEffectType     core.LinkStore[*checker.Type, *Effect]
	EffectSubtype        core.LinkStore[*checker.Type, *Effect]
	FiberType            core.LinkStore[*checker.Type, *Effect]
	EffectYieldableType  core.LinkStore[*checker.Type, *Effect]
	HasEffectTypeId      core.LinkStore[*checker.Type, bool]
	LayerType            core.LinkStore[*checker.Type, *Layer]
	ServiceType          core.LinkStore[*checker.Type, *Service]
	ContextTag           core.LinkStore[*checker.Type, *Service]
	IsSchemaType         core.LinkStore[*checker.Type, bool]
	EffectSchemaTypes    core.LinkStore[*checker.Type, *SchemaTypes]
	IsScopeType          core.LinkStore[*checker.Type, bool]
	IsPipeableType       core.LinkStore[*checker.Type, bool]
	PromiseType          core.LinkStore[*checker.Type, *checker.Type]
	IsGlobalErrorType    core.LinkStore[*checker.Type, bool]
	IsYieldableErrorType core.LinkStore[*checker.Type, bool]

	ExtendsContextTag          core.LinkStore[*ast.Node, *ContextTagResult]
	ExtendsDataTaggedError     core.LinkStore[*ast.Node, *DataTaggedErrorResult]
	ExtendsEffectModelClass    core.LinkStore[*ast.Node, *EffectModelClassResult]
	ExtendsEffectService       core.LinkStore[*ast.Node, *EffectServiceResult]
	ExtendsEffectTag           core.LinkStore[*ast.Node, *EffectTagResult]
	ExtendsSchemaClass         core.LinkStore[*ast.Node, *SchemaClassResult]
	ExtendsSchemaRequestClass  core.LinkStore[*ast.Node, *SchemaClassResult]
	ExtendsSchemaTaggedClass   core.LinkStore[*ast.Node, *SchemaTaggedResult]
	ExtendsSchemaTaggedError   core.LinkStore[*ast.Node, *SchemaTaggedResult]
	ExtendsSchemaTaggedRequest core.LinkStore[*ast.Node, *SchemaTaggedResult]
	ExtendsServiceMapService   core.LinkStore[*ast.Node, *ServiceMapServiceResult]
	ExtendsEffectSqlModelClass core.LinkStore[*ast.Node, *SqlModelClassResult]

	EffectGenCall                core.LinkStore[*ast.Node, *EffectGenCallResult]
	EffectFnCall                 core.LinkStore[*ast.Node, *EffectFnCallResult]
	ParseEffectFnOpportunity     core.LinkStore[*ast.Node, *EffectFnOpportunityResult]
	ParsePipeCall                core.LinkStore[*ast.Node, *ParsedPipeCallResult]
	ExecutionFlow                core.LinkStore[*ast.SourceFile, *ExecutionFlow]
	EffectContextFlags           core.LinkStore[*ast.Node, EffectContextFlags]
	EffectYieldGeneratorFunction core.LinkStore[*ast.Node, *ast.FunctionExpression]

	discoverPackagesComputed    bool
	discoverPackagesValue       []DiscoveredPackage
	detectEffectVersionComputed bool
	detectEffectVersionValue    EffectMajorVersion
	PackageJsonForSourceFile    core.LinkStore[*ast.SourceFile, *packagejson.PackageJson]
	EffectContextAnalyzed       core.LinkStore[*ast.SourceFile, bool]
	ExpectedAndRealTypes        core.LinkStore[*ast.SourceFile, []ExpectedAndRealType]
	PipingFlowsWithEffectFn     core.LinkStore[*ast.SourceFile, []*PipingFlow]
	PipingFlowsWithoutEffectFn  core.LinkStore[*ast.SourceFile, []*PipingFlow]
}

// Cached checks the store for an existing value. On miss, it calls compute,
// stores the result, and returns it. This correctly caches zero/nil values
// as valid negative results.
func Cached[K comparable, V any](store *core.LinkStore[K, V], key K, compute func() V) V {
	if store.Has(key) {
		return *store.TryGet(key)
	}
	value := compute()
	*store.Get(key) = value
	return value
}

// NewTypeParser builds a checker-backed TypeParser.
func NewTypeParser(p checker.Program, c *checker.Checker) *TypeParser {
	if p == nil {
		panic("typeparser.NewTypeParser: nil program")
	}
	if c == nil {
		panic("typeparser.NewTypeParser: nil checker")
	}
	if c.EffectLinks == nil {
		c.EffectLinks = &EffectLinks{}
	}
	return &TypeParser{program: p, checker: c, links: c.EffectLinks.(*EffectLinks)}
}
