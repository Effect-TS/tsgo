package typeparser

import (
	"github.com/effect-ts/tsgo/internal/importstyle"
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

type denseNodeStore[V any] = checker.NodeLinkStore[V]

// EffectLinks holds per-checker cached type-parser results.
// One instance is lazily created per Checker and cached on TypeParser.
type EffectLinks struct {
	TypeAtLocation               denseNodeStore[*checker.Type]
	ReferenceSymbol              denseNodeStore[*ast.Symbol]
	EffectContextFlags           denseNodeStore[EffectContextFlags]
	EffectYieldGeneratorFunction denseNodeStore[*ast.FunctionExpression]
	DataFirstOrLastCall          denseNodeStore[*ParsedDataFirstOrLastCall]
	CheckedExpressionType        denseNodeStore[*checker.Type]
	checkedExpressionTypesPaused bool
	// checkedExpressionTypeSourceFile is the last source file found type
	// checked by checkedExpressionType.
	checkedExpressionTypeSourceFile *ast.SourceFile

	ObjectLiteralContextualType               core.LinkStore[*ast.Node, *checker.Type]
	ObjectLiteralConstraintContextualType     core.LinkStore[*ast.Node, *checker.Type]
	ObjectLiteralBareConstraintContextualType core.LinkStore[*ast.Node, *checker.Type]

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
	ScopeType              core.LinkStore[*checker.Type, *checker.Type]
	IsPipeableType         core.LinkStore[*checker.Type, bool]
	GlobalInterfaceHasPipe core.LinkStore[string, bool]
	PromiseType            core.LinkStore[*checker.Type, *checker.Type]
	IsGlobalErrorType      core.LinkStore[*checker.Type, bool]
	YieldableErrorType     core.LinkStore[*checker.Type, *checker.Type]
	ModuleExportIdentities checker.SymbolLinkStore[[]moduleExportIdentity]
	ModuleExportFilter     checker.SymbolLinkStore[moduleExportFilter]
	ModuleExportNames      core.LinkStore[*ast.SourceFile, moduleExportNames]
	ModuleFileFilter       core.LinkStore[*ast.SourceFile, moduleExportFilter]
	PipeableSignatureShape core.LinkStore[pipeableSignatureShapeCacheKey, bool]
	nativeArrayRegistry    *nativeArrayRegistry
	barrelReader           *importstyle.BarrelReader
	nativeArrayMember      core.LinkStore[*ast.Symbol, nativeArrayMember]
	nativeObjectRegistry   *nativeObjectRegistry
	nativeObjectMember     core.LinkStore[*ast.Symbol, nativeObjectMember]
	nativeStringRegistry   *nativeStringRegistry
	nativeStringMember     core.LinkStore[*ast.Symbol, nativeStringMember]
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
	ExtendsDataError           core.LinkStore[*ast.Node, *DataErrorResult]
	ExtendsDataTaggedClass     core.LinkStore[*ast.Node, *DataTaggedResult]
	ExtendsDataTaggedError     core.LinkStore[*ast.Node, *DataTaggedResult]
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

	EffectGenCall            core.LinkStore[*ast.Node, *EffectGenCallResult]
	EffectFnCall             core.LinkStore[*ast.Node, *EffectFnCallResult]
	ParseEffectFnOpportunity core.LinkStore[*ast.Node, *EffectFnOpportunityResult]
	ParsePipeCall            core.LinkStore[*ast.Node, *ParsedPipeCallResult]

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
	if value := store.TryGet(key); value != nil {
		return *value
	}
	value := compute()
	*store.Get(key) = value
	return value
}

// CachedNode is Cached for node-keyed stores.
func CachedNode[V any](store *checker.NodeLinkStore[V], node *ast.Node, compute func() V) V {
	if value := store.TryGet(node); value != nil {
		return *value
	}
	value := compute()
	*store.Get(node) = value
	return value
}

// NewTypeParser builds a checker-backed TypeParser.
// BarrelReader returns the checker's barrel cache, or nil when the program
// cannot read files.
func (tp *TypeParser) BarrelReader() *importstyle.BarrelReader {
	if tp.links.barrelReader == nil {
		program, ok := tp.program.(importstyle.ProgramFiles)
		if !ok {
			return nil
		}
		tp.links.barrelReader = importstyle.NewProgramBarrelReader(program)
	}
	return tp.links.barrelReader
}

func NewTypeParser(p checker.Program, c *checker.Checker) *TypeParser {
	if p == nil {
		panic("typeparser.NewTypeParser: nil program")
	}
	if c == nil {
		panic("typeparser.NewTypeParser: nil checker")
	}
	return &TypeParser{program: p, checker: c, links: effectLinksOf(c)}
}

func effectLinksOf(c *checker.Checker) *EffectLinks {
	if c.EffectLinks == nil {
		c.EffectLinks = &EffectLinks{}
	}
	return c.EffectLinks.(*EffectLinks)
}
