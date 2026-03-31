package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/packagejson"
)

// TypeParser groups checker-backed typeparser operations behind a shared
// checker/program pair so callers do not need to thread them through each call.
type TypeParser struct {
	program checker.Program
	checker *checker.Checker
}

// NewTypeParser builds a checker-backed TypeParser.
func NewTypeParser(p checker.Program, c *checker.Checker) TypeParser {
	if p == nil {
		panic("typeparser.NewTypeParser: nil program")
	}
	if c == nil {
		panic("typeparser.NewTypeParser: nil checker")
	}
	return TypeParser{program: p, checker: c}
}

func (tp TypeParser) DetectEffectVersion() EffectMajorVersion {
	return DetectEffectVersion(tp.checker)
}

func (tp TypeParser) SupportedEffectVersion() EffectMajorVersion {
	return SupportedEffectVersion(tp.checker)
}

func (tp TypeParser) DetectEffectVersionString() string {
	return DetectEffectVersionString(tp.checker)
}

func (tp TypeParser) DiscoverPackages() []DiscoveredPackage {
	return DiscoverPackages(tp.checker)
}

func (tp TypeParser) IsYieldableErrorType(t *checker.Type) bool {
	return IsYieldableErrorType(tp.checker, t)
}

func (tp TypeParser) PackageJsonForSourceFile(sf *ast.SourceFile) *packagejson.PackageJson {
	return PackageJsonForSourceFile(tp.checker, sf)
}

func (tp TypeParser) AppendToUniqueTypesMap(memory map[string]*checker.Type, initialType *checker.Type, shouldExclude func(*checker.Type) bool) UniqueTypesResult {
	return AppendToUniqueTypesMap(tp.checker, memory, initialType, shouldExclude)
}

func (tp TypeParser) ParseEffectFnIife(node *ast.Node) *EffectFnIifeResult {
	return ParseEffectFnIife(tp.checker, node)
}

func (tp TypeParser) ExtendsSchemaTaggedClass(classNode *ast.Node) *SchemaTaggedResult {
	return ExtendsSchemaTaggedClass(tp.checker, classNode)
}

func (tp TypeParser) ExtendsSchemaTaggedError(classNode *ast.Node) *SchemaTaggedResult {
	return ExtendsSchemaTaggedError(tp.checker, classNode)
}

func (tp TypeParser) ExtendsSchemaTaggedRequest(classNode *ast.Node) *SchemaTaggedResult {
	return ExtendsSchemaTaggedRequest(tp.checker, classNode)
}

func (tp TypeParser) ExtendsEffectTag(classNode *ast.Node) *EffectTagResult {
	return ExtendsEffectTag(tp.checker, classNode)
}

func (tp TypeParser) ExtendsDataTaggedError(classNode *ast.Node) *DataTaggedErrorResult {
	return ExtendsDataTaggedError(tp.checker, classNode)
}

func (tp TypeParser) ExtendsEffectModelClass(classNode *ast.Node) *EffectModelClassResult {
	return ExtendsEffectModelClass(tp.checker, classNode)
}

func (tp TypeParser) ServiceType(t *checker.Type, atLocation *ast.Node) *Service {
	return ServiceType(tp.checker, t, atLocation)
}

func (tp TypeParser) IsServiceType(t *checker.Type, atLocation *ast.Node) bool {
	return IsServiceType(tp.checker, t, atLocation)
}

func (tp TypeParser) ContextTag(t *checker.Type, atLocation *ast.Node) *Service {
	return ContextTag(tp.checker, t, atLocation)
}

func (tp TypeParser) IsContextTag(t *checker.Type, atLocation *ast.Node) bool {
	return IsContextTag(tp.checker, t, atLocation)
}

func (tp TypeParser) ExtendsContextTag(classNode *ast.Node) *ContextTagResult {
	return ExtendsContextTag(tp.checker, classNode)
}

func (tp TypeParser) IsGlobalErrorType(t *checker.Type) bool {
	return IsGlobalErrorType(tp.checker, t)
}

func (tp TypeParser) IsScopeType(t *checker.Type, atLocation *ast.Node) bool {
	return IsScopeType(tp.checker, t, atLocation)
}

func (tp TypeParser) ExtendsSchemaClass(classNode *ast.Node) *SchemaClassResult {
	return ExtendsSchemaClass(tp.checker, classNode)
}

func (tp TypeParser) ExtendsSchemaRequestClass(classNode *ast.Node) *SchemaClassResult {
	return ExtendsSchemaRequestClass(tp.checker, classNode)
}

func (tp TypeParser) ExtendsEffectService(classNode *ast.Node) *EffectServiceResult {
	return ExtendsEffectService(tp.checker, classNode)
}

func (tp TypeParser) IsPipeableType(t *checker.Type, atLocation *ast.Node) bool {
	return IsPipeableType(tp.checker, t, atLocation)
}

func (tp TypeParser) IsSafelyPipeableCallee(callee *ast.Node) bool {
	return IsSafelyPipeableCallee(tp.checker, callee)
}

func (tp TypeParser) EffectFnGenCall(node *ast.Node) *EffectGenCallResult {
	return EffectFnGenCall(tp.checker, node)
}

func (tp TypeParser) IsInsideEffectFn(fnNode *ast.Node) bool {
	return IsInsideEffectFn(tp.checker, fnNode)
}

func (tp TypeParser) ParseEffectFnOpportunity(node *ast.Node) *EffectFnOpportunityResult {
	return ParseEffectFnOpportunity(tp.checker, node)
}

func (tp TypeParser) IsNodeReferenceToServiceMapModuleApi(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToServiceMapModuleApi(tp.checker, node, memberName)
}

func (tp TypeParser) IsNodeReferenceToEffectContextModuleApi(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToEffectContextModuleApi(tp.checker, node, memberName)
}

func (tp TypeParser) ParsePipeCall(node *ast.Node) *ParsedPipeCallResult {
	return ParsePipeCall(tp.checker, node)
}

func (tp TypeParser) PipingFlows(sf *ast.SourceFile, includeEffectFn bool) []*PipingFlow {
	return PipingFlows(tp.checker, sf, includeEffectFn)
}

func (tp TypeParser) IsNodeReferenceToEffectDataModuleApi(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToEffectDataModuleApi(tp.checker, node, memberName)
}

func (tp TypeParser) IsNodeReferenceToEffectModelModuleApi(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToEffectModelModuleApi(tp.checker, node, memberName)
}

func (tp TypeParser) ExtendsServiceMapService(classNode *ast.Node) *ServiceMapServiceResult {
	return ExtendsServiceMapService(tp.checker, classNode)
}

func (tp TypeParser) ExpectedAndRealTypes(sf *ast.SourceFile) []ExpectedAndRealType {
	return ExpectedAndRealTypes(tp.checker, sf)
}

func (tp TypeParser) EffectFnUntracedEagerGenCall(node *ast.Node) *EffectGenCallResult {
	return EffectFnUntracedEagerGenCall(tp.checker, node)
}

func (tp TypeParser) LayerType(t *checker.Type, atLocation *ast.Node) *Layer {
	return LayerType(tp.checker, t, atLocation)
}

func (tp TypeParser) IsLayerType(t *checker.Type, atLocation *ast.Node) bool {
	return IsLayerType(tp.checker, t, atLocation)
}

func (tp TypeParser) IsNodeReferenceToEffectLayerModuleApi(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToEffectLayerModuleApi(tp.checker, node, memberName)
}

func (tp TypeParser) GetTypeAtLocation(node *ast.Node) *checker.Type {
	return GetTypeAtLocation(tp.checker, node)
}

func (tp TypeParser) EffectYieldableType(t *checker.Type, atLocation *ast.Node) *Effect {
	return EffectYieldableType(tp.checker, t, atLocation)
}

func (tp TypeParser) EffectType(t *checker.Type, atLocation *ast.Node) *Effect {
	return EffectType(tp.checker, t, atLocation)
}

func (tp TypeParser) IsEffectType(t *checker.Type, atLocation *ast.Node) bool {
	return IsEffectType(tp.checker, t, atLocation)
}

func (tp TypeParser) StrictEffectType(t *checker.Type, atLocation *ast.Node) *Effect {
	return StrictEffectType(tp.checker, t, atLocation)
}

func (tp TypeParser) StrictIsEffectType(t *checker.Type, atLocation *ast.Node) bool {
	return StrictIsEffectType(tp.checker, t, atLocation)
}

func (tp TypeParser) EffectSubtype(t *checker.Type, atLocation *ast.Node) *Effect {
	return EffectSubtype(tp.checker, t, atLocation)
}

func (tp TypeParser) IsEffectSubtype(t *checker.Type, atLocation *ast.Node) bool {
	return IsEffectSubtype(tp.checker, t, atLocation)
}

func (tp TypeParser) FiberType(t *checker.Type, atLocation *ast.Node) *Effect {
	return FiberType(tp.checker, t, atLocation)
}

func (tp TypeParser) IsFiberType(t *checker.Type, atLocation *ast.Node) bool {
	return IsFiberType(tp.checker, t, atLocation)
}

func (tp TypeParser) HasEffectTypeId(t *checker.Type, atLocation *ast.Node) bool {
	return HasEffectTypeId(tp.checker, t, atLocation)
}

func (tp TypeParser) IsExpressionEffectModule(node *ast.Node) bool {
	return IsExpressionEffectModule(tp.checker, node)
}

func (tp TypeParser) IsNodeReferenceToEffectModuleApi(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToEffectModuleApi(tp.checker, node, memberName)
}

func (tp TypeParser) IsNodeReferenceToEffectPackageExport(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToEffectPackageExport(tp.checker, node, memberName)
}

func (tp TypeParser) GetPropertyOfTypeByName(t *checker.Type, name string) *ast.Symbol {
	return GetPropertyOfTypeByName(tp.checker, t, name)
}

func (tp TypeParser) ResolveToGlobalSymbol(sym *ast.Symbol) *ast.Symbol {
	return ResolveToGlobalSymbol(tp.checker, sym)
}

func (tp TypeParser) EffectFnUntracedGenCall(node *ast.Node) *EffectGenCallResult {
	return EffectFnUntracedGenCall(tp.checker, node)
}

func (tp TypeParser) ExtendsEffectSqlModelClass(classNode *ast.Node) *SqlModelClassResult {
	return ExtendsEffectSqlModelClass(tp.checker, classNode)
}

func (tp TypeParser) ReferenceSymbolAtNode(node *ast.Node) *ast.Symbol {
	return ReferenceSymbolAtNode(tp.checker, node)
}

func (tp TypeParser) IsSourceFileInPackage(sf *ast.SourceFile, packageName string) bool {
	return IsSourceFileInPackage(tp.checker, sf, packageName)
}

func (tp TypeParser) IsNodeReferenceToModuleExport(node *ast.Node, desc PackageSourceFileDescriptor, memberName string) bool {
	return IsNodeReferenceToModuleExport(tp.checker, node, desc, memberName)
}

func (tp TypeParser) IsNodeReferenceToModule(node *ast.Node, desc PackageSourceFileDescriptor) bool {
	return IsNodeReferenceToModule(tp.checker, node, desc)
}

func (tp TypeParser) EffectGenCall(node *ast.Node) *EffectGenCallResult {
	return EffectGenCall(tp.checker, node)
}

func (tp TypeParser) GetEffectLinks() *EffectLinks {
	return GetEffectLinks(tp.checker)
}

func (tp TypeParser) IsSchemaType(t *checker.Type, atLocation *ast.Node) bool {
	return IsSchemaType(tp.checker, t, atLocation)
}

func (tp TypeParser) EffectSchemaTypes(t *checker.Type, atLocation *ast.Node) *SchemaTypes {
	return EffectSchemaTypes(tp.checker, t, atLocation)
}

func (tp TypeParser) IsNodeReferenceToEffectSchemaModuleApi(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToEffectSchemaModuleApi(tp.checker, node, memberName)
}

func (tp TypeParser) IsNodeReferenceToEffectParseResultModuleApi(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToEffectParseResultModuleApi(tp.checker, node, memberName)
}

func (tp TypeParser) IsNodeReferenceToEffectSchemaParserModuleApi(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToEffectSchemaParserModuleApi(tp.checker, node, memberName)
}

func (tp TypeParser) GetEffectContextFlags(node *ast.Node) EffectContextFlags {
	return GetEffectContextFlags(tp.checker, node)
}

func (tp TypeParser) GetEffectYieldGeneratorFunction(node *ast.Node) *ast.FunctionExpression {
	return GetEffectYieldGeneratorFunction(tp.checker, node)
}

func (tp TypeParser) EffectFnCall(node *ast.Node) *EffectFnCallResult {
	return EffectFnCall(tp.checker, node)
}

func (tp TypeParser) IsNodeReferenceToEffectSqlModelModuleApi(node *ast.Node, memberName string) bool {
	return IsNodeReferenceToEffectSqlModelModuleApi(tp.checker, node, memberName)
}
