package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/packagejson"
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
	transientAnalysisDepth int
	TypeAtLocation         core.LinkStore[*ast.Node, *checker.Type]
	EffectType             core.LinkStore[*checker.Type, *Effect]
	StreamType             core.LinkStore[*checker.Type, *Effect]
	StrictEffectType       core.LinkStore[*checker.Type, *Effect]
	EffectSubtype          core.LinkStore[*checker.Type, *Effect]
	FiberType              core.LinkStore[*checker.Type, *Effect]
	EffectYieldableType    core.LinkStore[*checker.Type, *Effect]
	HasEffectTypeId        core.LinkStore[*checker.Type, bool]
	LayerType              core.LinkStore[*checker.Type, *Layer]
	ServiceType            core.LinkStore[*checker.Type, *Service]
	ContextTag             core.LinkStore[*checker.Type, *Service]
	EffectSchemaTypes      core.LinkStore[*checker.Type, *SchemaTypes]
	IsScopeType            core.LinkStore[*checker.Type, bool]
	IsPipeableType         core.LinkStore[*checker.Type, bool]
	PromiseType            core.LinkStore[*checker.Type, *checker.Type]
	IsGlobalErrorType      core.LinkStore[*checker.Type, bool]
	IsYieldableErrorType   core.LinkStore[*checker.Type, bool]
	ReferenceSymbol        core.LinkStore[*ast.Node, *ast.Symbol]
	ModuleExportReference  core.LinkStore[moduleExportReferenceCacheKey, bool]
	PipeableSignatureShape core.LinkStore[pipeableSignatureShapeCacheKey, bool]
	nativeArrayRegistry    *nativeArrayRegistry
	nativeArrayMember      core.LinkStore[*ast.Symbol, nativeArrayMember]
	nativeObjectRegistry   *nativeObjectRegistry
	nativeObjectMember     core.LinkStore[*ast.Symbol, nativeObjectMember]
	// API-stability caches. Declared lookups persist per checker: the declared
	// stability of a symbol, of a raw signature overload and of one
	// declaration, shared with the unstableApiUsage/experimentalApiUsage rules.
	// Declared lookups never trigger computed analysis. A computed stability
	// surface lives in an analysis-local ApiStabilitySession, and every
	// complete, settled, context-free concrete type or signature surface is
	// additionally published here as an immutable snapshot so a later export,
	// file or TypeParser over the same checker composes it instead of
	// recomputing. The snapshot excludes the component's own declared tag,
	// which stays in the declared caches and is re-added by each consumer; a
	// substitution-context or incomplete/blocked result is never published and
	// stays analysis-local. Ceilings, locations and diagnostics are never
	// cached.
	ApiStabilityDeclaredSymbol      core.LinkStore[*ast.Symbol, ApiStabilityDeclaration]
	ApiStabilityDeclaredSignature   core.LinkStore[*checker.Signature, ApiStabilityDeclaration]
	ApiStabilityDeclaredDeclaration core.LinkStore[*ast.Node, ApiStabilityDeclaration]
	ApiStabilitySurfaceType         core.LinkStore[apiStabilitySurfaceTypeKey, apiStabilitySharedSurface]
	ApiStabilitySurfaceSignature    core.LinkStore[*checker.Signature, apiStabilitySharedSurface]

	ExtendsContextTag          core.LinkStore[*ast.Node, *ContextTagResult]
	ExtendsDataTaggedError     core.LinkStore[*ast.Node, *DataTaggedErrorResult]
	ExtendsEffectModelClass    core.LinkStore[*ast.Node, *EffectModelClassResult]
	ExtendsEffectService       core.LinkStore[*ast.Node, *EffectServiceResult]
	ExtendsEffectTag           core.LinkStore[*ast.Node, *EffectTagResult]
	ExtendsSchemaClass         core.LinkStore[*ast.Node, *SchemaClassResult]
	ExtendsSchemaError         core.LinkStore[*ast.Node, *SchemaClassResult]
	ExtendsSchemaOpaque        core.LinkStore[*ast.Node, *SchemaOpaqueResult]
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
	ApiStabilityUsages          core.LinkStore[*ast.SourceFile, []ApiStabilityUsage]
	PipingFlowsWithEffectFn     core.LinkStore[*ast.SourceFile, []*PipingFlow]
	PipingFlowsWithoutEffectFn  core.LinkStore[*ast.SourceFile, []*PipingFlow]
}

// BeginTransientAnalysis retains file-analysis caches until the outermost
// analysis completes. CLI diagnostics do not need these results afterwards;
// symbol/type metadata and package discovery remain shared across files.
func (tp *TypeParser) BeginTransientAnalysis() func() {
	links := tp.links
	links.transientAnalysisDepth++
	return func() {
		links.transientAnalysisDepth--
		if links.transientAnalysisDepth != 0 {
			return
		}
		links.TypeAtLocation = core.LinkStore[*ast.Node, *checker.Type]{}
		links.ReferenceSymbol = core.LinkStore[*ast.Node, *ast.Symbol]{}
		links.EffectGenCall = core.LinkStore[*ast.Node, *EffectGenCallResult]{}
		links.EffectFnCall = core.LinkStore[*ast.Node, *EffectFnCallResult]{}
		links.ParseEffectFnOpportunity = core.LinkStore[*ast.Node, *EffectFnOpportunityResult]{}
		links.ParsePipeCall = core.LinkStore[*ast.Node, *ParsedPipeCallResult]{}
		links.ExecutionFlow = core.LinkStore[*ast.SourceFile, *ExecutionFlow]{}
		links.EffectContextFlags = core.LinkStore[*ast.Node, EffectContextFlags]{}
		links.EffectYieldGeneratorFunction = core.LinkStore[*ast.Node, *ast.FunctionExpression]{}
		links.EffectContextAnalyzed = core.LinkStore[*ast.SourceFile, bool]{}
		links.ExpectedAndRealTypes = core.LinkStore[*ast.SourceFile, []ExpectedAndRealType]{}
		links.ApiStabilityUsages = core.LinkStore[*ast.SourceFile, []ApiStabilityUsage]{}
		links.PipingFlowsWithEffectFn = core.LinkStore[*ast.SourceFile, []*PipingFlow]{}
		links.PipingFlowsWithoutEffectFn = core.LinkStore[*ast.SourceFile, []*PipingFlow]{}
	}
}

// Cached checks the store for an existing value. On miss, it calls compute,
// stores the result, and returns it. This correctly caches zero/nil values
// as valid negative results.
func Cached[K comparable, V any](store *core.LinkStore[K, V], key K, compute func() V) V {
	if value := store.TryGet(key); value != nil {
		return *value
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
