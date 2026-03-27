package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/packagejson"
)

// EffectLinks holds per-checker cached type-parser results.
// One instance is lazily created per Checker and stored in its EffectLinks field.
type EffectLinks struct {
	TypeAtLocation       core.LinkStore[*ast.Node, *checker.Type]
	EffectType           core.LinkStore[*checker.Type, *Effect]
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
	IsGlobalErrorType    core.LinkStore[*checker.Type, bool]
	IsYieldableErrorType core.LinkStore[*checker.Type, bool]

	// Node-keyed Extends* parsers
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

	// Node-keyed call-site parsers
	EffectGenCall                core.LinkStore[*ast.Node, *EffectGenCallResult]
	EffectFnCall                 core.LinkStore[*ast.Node, *EffectFnCallResult]
	EffectFnGenCall              core.LinkStore[*ast.Node, *EffectGenCallResult]
	EffectFnUntracedGenCall      core.LinkStore[*ast.Node, *EffectGenCallResult]
	EffectFnUntracedEagerGenCall core.LinkStore[*ast.Node, *EffectGenCallResult]
	ParseEffectFnIife            core.LinkStore[*ast.Node, *EffectFnIifeResult]
	ParseEffectFnOpportunity     core.LinkStore[*ast.Node, *EffectFnOpportunityResult]
	ParsePipeCall                core.LinkStore[*ast.Node, *ParsedPipeCallResult]
	EffectContextFlags           core.LinkStore[*ast.Node, EffectContextFlags]
	EffectYieldGeneratorFunction core.LinkStore[*ast.Node, *ast.FunctionExpression]

	// Checker-level cached scalar values
	discoverPackagesComputed       bool
	discoverPackagesValue          []DiscoveredPackage
	detectEffectVersionComputed    bool
	detectEffectVersionValue       EffectMajorVersion
	supportedEffectVersionComputed bool
	supportedEffectVersionValue    EffectMajorVersion

	// SourceFile-keyed aggregate parsers
	PackageJsonForSourceFile   core.LinkStore[*ast.SourceFile, *packagejson.PackageJson]
	EffectContextAnalyzed      core.LinkStore[*ast.SourceFile, bool]
	ExpectedAndRealTypes       core.LinkStore[*ast.SourceFile, []ExpectedAndRealType]
	PipingFlowsWithEffectFn    core.LinkStore[*ast.SourceFile, []*PipingFlow]
	PipingFlowsWithoutEffectFn core.LinkStore[*ast.SourceFile, []*PipingFlow]
}

// GetEffectLinks returns the EffectLinks instance attached to the given checker,
// lazily creating and storing it on first access.
func GetEffectLinks(c *checker.Checker) *EffectLinks {
	if c.EffectLinks == nil {
		c.EffectLinks = &EffectLinks{}
	}
	return c.EffectLinks.(*EffectLinks)
}
