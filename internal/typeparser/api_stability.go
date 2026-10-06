package typeparser

import (
	"maps"
	"slices"
	"sort"
	"strconv"
	"strings"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// ApiStabilityLevel orders the Effect stability tiers. A higher level is less
// stable, so a public surface may only expose dependencies whose level is at
// most the level declared for the export that owns the surface.
type ApiStabilityLevel int

const (
	ApiStabilityStable ApiStabilityLevel = iota
	ApiStabilityUnstable
	ApiStabilityExperimental
)

// ApiStabilityLevelTag returns the `@stability` tag spelling for a level as
// rendered by the usage diagnostics. The stable tier is spelled as the empty
// string because it is the implicit default; an explicit `@stability stable`
// tag is still parsed (see stabilityOfDeclarationTag) and keeps its declaration
// as provenance, so the declared accessors distinguish it from an absent tag
// even though this spelling helper returns the same empty string for both.
func ApiStabilityLevelTag(level ApiStabilityLevel) string {
	switch level {
	case ApiStabilityUnstable:
		return "unstable"
	case ApiStabilityExperimental:
		return "experimental"
	default:
		return ""
	}
}

// ApiStabilityLevelFromTag maps a `@stability` tag spelling to a level. An
// unrecognized or empty tag is stable.
func ApiStabilityLevelFromTag(tag string) ApiStabilityLevel {
	return apiStabilityRank(tag)
}

// ApiStabilityDeclaration is a declared stability tier together with the
// declaration that carries the tag. The declaration is retained for callers
// that need provenance (allow-list naming and diagnostic locations). An
// explicit `@stability stable` tag keeps its declaration, distinct from an
// untagged default-stable symbol, which has no declaration.
type ApiStabilityDeclaration struct {
	Level       ApiStabilityLevel
	Declaration *ast.Node
}

// ApiStabilityDependency names one exposed component of a public API surface
// whose declared stability is above stable. The offender is either a symbol
// (properties, methods, referenced types) or a signature declaration; the
// declaration carries the `@stability` provenance used for reporting.
type ApiStabilityDependency struct {
	Symbol      *ast.Symbol
	Signature   *checker.Signature
	Declaration *ast.Node
	Level       ApiStabilityLevel
}

// ApiStabilityUsed is the computed stability of the components a surface
// exposes: the minimum guaranteed stability (the least stable dependency found,
// stable when none was found) and the offending components themselves.
//
// The surface of a root component (a symbol, a represented type or a signature)
// excludes the root's own declared tag. Declared stability is a separate
// lookup: DeclaredApiStabilityOfSymbol and DeclaredApiStabilityOfSignature
// return the component's own tag and its declaration, are cached per checker,
// and never trigger a computed surface. The same component reached as a child
// of another root contributes its own tag to that parent surface; only the
// root conversion removes it, so the exclusion is a result semantic and never
// a traversal stop.
//
// Incomplete reports that the analysis could not establish every component of
// the surface (a component is unavailable or a guarded read was refused), so
// the dependency list may be missing findings and must not be treated as a
// stable result; the incomplete result is discarded with its session and a
// later export or call retries.
type ApiStabilityUsed struct {
	Minimum      ApiStabilityLevel
	Dependencies []ApiStabilityDependency
	Incomplete   bool
}

// apiStabilityInspection distinguishes how a represented type is inspected. A
// named class or interface reached as a dependency stays shallow: its symbol
// and its public call/construct signatures are inspected, but its members are
// not recursively expanded. The direct surface of an export is expanded: own
// and inherited members, index signatures and signatures are all inspected.
type apiStabilityInspection int

const (
	apiStabilityShallow apiStabilityInspection = iota
	apiStabilityExpand
)

// apiStabilityFindingKey identifies one offender inside a surface. A symbol
// finding and a signature finding are distinct, so a tagged overload of a
// stable symbol is reported even though the symbol itself is stable. A
// declaration finding names an offender that has no materialized symbol or
// signature (a tagged member of a declaration annotation), so its declaration
// is the identity.
type apiStabilityFindingKey struct {
	symbol      *ast.Symbol
	signature   *checker.Signature
	declaration *ast.Node
}

type apiStabilityFinding struct {
	level       ApiStabilityLevel
	declaration *ast.Node
}

// apiStabilitySurface is a memoized set of stability findings. complete is
// false when the result was cut by an in-progress cycle or by a component the
// checker has not materialized; such a result is never treated as a finished
// memo entry. cutCycle marks a result that was cut only by cycle back-edges,
// which the settle fixpoint can establish as complete; blocked marks a result
// whose components are unavailable or whose compiler outcome must not be
// guessed, and which is never promoted.
//
// observedEpoch is analysis-local settlement bookkeeping. It records the
// analysis materialization epoch at which this result was last collected or
// recollected, so the settle fixpoint can tell whether a blocked result may
// have become collectable since (a later analysis read may have materialized a
// component the guard had refused). It is never part of equality or merge
// semantics: complete results ignore it, and blocked results are never
// promoted.
type apiStabilitySurface struct {
	findings map[apiStabilityFindingKey]apiStabilityFinding
	complete bool
	cutCycle bool
	blocked  bool

	observedEpoch int
}

func newApiStabilitySurface() apiStabilitySurface {
	return apiStabilitySurface{findings: make(map[apiStabilityFindingKey]apiStabilityFinding), complete: true}
}

// cycleCutSurface marks a result cut by an in-progress cycle. The settle
// fixpoint can promote it to complete once findings stop changing, because the
// back-edge's own direct findings are collected when the cut target is first
// entered.
func cycleCutSurface() apiStabilitySurface {
	return apiStabilitySurface{findings: make(map[apiStabilityFindingKey]apiStabilityFinding), cutCycle: true}
}

// blockedSurface marks a result whose components are not materialized or whose
// represented outcome is unavailable. It is never promoted to complete, so a
// later query or session against a checked checker recomputes it.
func blockedSurface() apiStabilitySurface {
	return apiStabilitySurface{findings: make(map[apiStabilityFindingKey]apiStabilityFinding), blocked: true}
}

// cutByCycle marks a surface that an in-progress cycle back-edge cut short. The
// settle fixpoint can establish it as complete once findings stop changing.
func (s *apiStabilitySurface) cutByCycle() {
	s.complete = false
	s.cutCycle = true
}

// block marks a surface whose remaining components are unavailable. A blocked
// surface is never promoted to complete, so a later query or session against a
// materialized checker recomputes it.
func (s *apiStabilitySurface) block() {
	s.complete = false
	s.blocked = true
}

func (s *apiStabilitySurface) merge(other apiStabilitySurface) {
	if !other.complete {
		s.complete = false
		s.cutCycle = s.cutCycle || other.cutCycle
		s.blocked = s.blocked || other.blocked
	}
	for key, finding := range other.findings {
		if existing, ok := s.findings[key]; !ok || finding.level > existing.level {
			s.findings[key] = finding
		}
	}
}

// equals compares the semantic content of two surfaces. observedEpoch is
// deliberately excluded: it is settlement bookkeeping, so updating it after a
// recollection is not a fixpoint change.
func (s apiStabilitySurface) equals(other apiStabilitySurface) bool {
	return s.complete == other.complete && s.cutCycle == other.cutCycle && s.blocked == other.blocked &&
		maps.Equal(s.findings, other.findings)
}

// apiStabilityCarrier is the interned, concrete enclosing identity of an
// internal raw-component traversal. It is built only from concrete compiler
// objects: a reference or instantiated owner type, a concrete signature, and
// the concrete argument types of a represented generic application. Interning
// in the analysis-local carrier table makes equal chains share one node
// pointer, so session and cycle keys compare by concrete identity instead of a
// serialized substitution context. No context string and no raw-target-only key
// ever participates.
type apiStabilityCarrier struct {
	parent    *apiStabilityCarrier
	owner     *checker.Type
	signature *checker.Signature

	// argument marks a carrier link that contributes one concrete type
	// argument. marker keeps the link distinct even when the argument itself is
	// unrepresented (nil), so two different applications never share a carrier
	// by accident.
	argument *checker.Type
	marker   bool
}

// apiStabilitySubstFrame is one link in a composed substitution context. A
// frame maps a signature's type parameters, a reference's type parameters, or
// a compiler-provided mapper to represented types. The chain keeps nested
// substitutions intact: a base type's argument can itself refer to the
// enclosing type's parameters. carrier is the interned concrete identity of
// the whole chain.
type apiStabilitySubstFrame struct {
	parent    *apiStabilitySubstFrame
	mapper    *checker.TypeMapper
	signature *checker.Signature

	// parameters and arguments bind a class or interface reference's type
	// parameters to the reference's represented type arguments. A nil argument
	// replaces the parameter without a represented value: dependencies of the
	// replaced parameter are unknown, so the traversal blocks instead of
	// falling back to the declaration's constraint or default.
	parameters []*checker.Type
	arguments  []*checker.Type

	carrier *apiStabilityCarrier
}

type apiStabilitySubstitution struct {
	frame *apiStabilitySubstFrame
}

// carrier returns the concrete enclosing identity of the whole substitution
// chain, or nil when the traversal started at a concrete top-level object.
func (s apiStabilitySubstitution) carrier() *apiStabilityCarrier {
	if s.frame == nil {
		return nil
	}
	return s.frame.carrier
}

// aliasArgumentsAreSelf reports whether an alias's recorded type arguments are
// exactly its own declared type parameters. The declared type of a generic
// alias is parameterized that way; such arguments are declaration operands and
// are never part of an application's exposed surface.
func (a *apiStabilityAnalysis) aliasArgumentsAreSelf(alias *checker.TypeAlias) bool {
	if alias == nil {
		return true
	}
	arguments := alias.TypeArguments()
	if len(arguments) == 0 {
		return true
	}
	symbol := alias.Symbol()
	if symbol == nil {
		return false
	}
	parameters := a.tp.checker.GetLocalTypeParametersOfClassOrInterfaceOrTypeAlias(symbol)
	if len(parameters) != len(arguments) {
		return false
	}
	for index := range arguments {
		if arguments[index] != parameters[index] {
			return false
		}
	}
	return true
}

// internCarrier interns one concrete enclosing identity link into the analysis.
// The table is discarded with the analysis, so no carrier survives an export.
func (a *apiStabilityAnalysis) internCarrier(parent *apiStabilityCarrier, owner *checker.Type, signature *checker.Signature) *apiStabilityCarrier {
	if owner == nil && signature == nil {
		return parent
	}
	key := apiStabilityCarrier{parent: parent, owner: owner, signature: signature}
	if existing := a.carriers[key]; existing != nil {
		return existing
	}
	carrier := key
	a.carriers[key] = &carrier
	return &carrier
}

// internArgumentCarrier interns one concrete type argument of a represented
// generic application. A nil argument still interns a distinct link because the
// marker keeps the position of an unrepresented argument part of the identity.
func (a *apiStabilityAnalysis) internArgumentCarrier(parent *apiStabilityCarrier, argument *checker.Type) *apiStabilityCarrier {
	key := apiStabilityCarrier{parent: parent, argument: argument, marker: true}
	if existing := a.carriers[key]; existing != nil {
		return existing
	}
	carrier := key
	a.carriers[key] = &carrier
	return &carrier
}

// extendParametersSubstitution pushes one frame binding a set of type
// parameters to represented arguments. The concrete owner is part of the
// interned carrier, so the same raw component analyzed under two different
// concrete applications never shares a cached result.
func (a *apiStabilityAnalysis) extendParametersSubstitution(subst apiStabilitySubstitution, owner *checker.Type, parameters, arguments []*checker.Type) apiStabilitySubstitution {
	carrier := a.internCarrier(subst.carrier(), owner, nil)
	for _, argument := range arguments {
		carrier = a.internArgumentCarrier(carrier, argument)
	}
	return apiStabilitySubstitution{frame: &apiStabilitySubstFrame{
		parent:     subst.frame,
		parameters: parameters,
		arguments:  arguments,
		carrier:    carrier,
	}}
}

// extendMapperSubstitution pushes one frame for a compiler-provided mapper
// attached to an instantiated type (an inferred anonymous object, an
// instantiated mapped type, or an instantiated conditional).
func (a *apiStabilityAnalysis) extendMapperSubstitution(subst apiStabilitySubstitution, owner *checker.Type, mapper *checker.TypeMapper) apiStabilitySubstitution {
	carrier := a.internCarrier(subst.carrier(), owner, nil)
	return apiStabilitySubstitution{frame: &apiStabilitySubstFrame{
		parent:  subst.frame,
		mapper:  mapper,
		carrier: carrier,
	}}
}

// extendSignatureSubstitution pushes one frame for a concrete signature. The
// raw signature's type parameters are resolved through the concrete
// signature's represented type arguments and its instantiated parameter
// mappers.
func (a *apiStabilityAnalysis) extendSignatureSubstitution(subst apiStabilitySubstitution, signature *checker.Signature) apiStabilitySubstitution {
	carrier := a.internCarrier(subst.carrier(), nil, signature)
	return apiStabilitySubstitution{frame: &apiStabilitySubstFrame{
		parent:    subst.frame,
		signature: signature,
		carrier:   carrier,
	}}
}

// Cache keys are concrete compiler identities. t and signature are the
// represented objects under inspection; carrier is the interned concrete
// enclosing identity when a raw (uninstantiated) component is walked inside a
// concrete context. No serialized substitution context is ever part of a key.
type apiStabilityTypeKey struct {
	t       *checker.Type
	inspect apiStabilityInspection
	carrier *apiStabilityCarrier
}

type apiStabilitySignatureKey struct {
	signature *checker.Signature
	carrier   *apiStabilityCarrier
}

type apiStabilitySymbolKey struct {
	symbol  *ast.Symbol
	carrier *apiStabilityCarrier
}

type apiStabilityAliasKey struct {
	t       *checker.Type
	carrier *apiStabilityCarrier
}

// apiStabilitySurfaceTypeKey is the concrete, context-free identity of one
// shared type-surface entry: the represented type pointer and the fixed
// inspection policy. The two policies occupy separate entry slots, so an
// expanded result can never contaminate a shallow one. No serialized context
// string and no substitution carrier participates; a surface computed under a
// substitution context stays analysis-local.
type apiStabilitySurfaceTypeKey struct {
	t       *checker.Type
	inspect apiStabilityInspection
}

// apiStabilitySharedSurface is one immutable snapshot of a complete, settled,
// context-free surface published on the per-checker EffectLinks. findings
// excludes every finding that matches the component's own declared tag (the
// same exclusion the root result conversion applies); removed records the
// concrete identities of those findings so a consumer can re-add them, with
// their declared level, from the per-checker declared caches. The snapshot is
// never mutated after publication: a consumer copies the findings into its own
// analysis-local surface before composing.
type apiStabilitySharedSurface struct {
	findings map[apiStabilityFindingKey]apiStabilityFinding
	removed  []apiStabilityFindingKey
}

// apiStabilityMaterializationReadKind identifies one lazy read operation whose
// materialization epoch advance is deduplicated per concrete identity.
type apiStabilityMaterializationReadKind uint8

const (
	apiStabilityMaterializationReadDeclaredType apiStabilityMaterializationReadKind = iota
	apiStabilityMaterializationReadSignatures
	apiStabilityMaterializationReadMembers
	apiStabilityMaterializationReadNodeType
)

// apiStabilityMaterializationReadKey is the concrete identity of one
// deduplicated lazy read: the operation and the symbol or annotation node it
// resolves. It only decides whether the materialization epoch advances once
// more; it is never a cache identity.
type apiStabilityMaterializationReadKey struct {
	kind   apiStabilityMaterializationReadKind
	symbol *ast.Symbol
	node   *ast.Node
}

// stabilityOfDeclarationTag resolves the `@stability` tag of a declaration. A
// variable's JSDoc is usually attached to its enclosing variable statement.
// This is the single declaration-level parser shared by the API usage rules and
// this analysis. An explicit `stable` tag is recognized and is distinct from an
// absent tag: it overrides an inherited tag instead of being ignored.
func stabilityOfDeclarationTag(declaration *ast.Node) string {
	if declaration == nil {
		return ""
	}
	nodes := []*ast.Node{declaration}
	if declaration.Parent != nil && declaration.Parent.Kind == ast.KindVariableDeclarationList &&
		declaration.Parent.Parent != nil && declaration.Parent.Parent.Kind == ast.KindVariableStatement {
		nodes = append(nodes, declaration.Parent.Parent)
	}
	for _, node := range nodes {
		if node.Flags&ast.NodeFlagsPossiblyContainsStabilityTag == 0 {
			continue
		}
		for _, doc := range node.JSDoc(nil) {
			if doc.AsJSDoc().Tags == nil {
				continue
			}
			for _, tag := range doc.AsJSDoc().Tags.Nodes {
				if tag.Kind != ast.KindJSDocUnknownTag || tag.TagName().Text() != "stability" {
					continue
				}
				comment := tag.AsJSDocUnknownTag().Comment
				if comment != nil && len(comment.Nodes) > 0 && comment.Nodes[0].Kind == ast.KindJSDocText {
					value := strings.TrimSpace(comment.Nodes[0].Text())
					if value == "stable" || value == "unstable" || value == "experimental" {
						return value
					}
				}
			}
		}
	}
	return ""
}

// StabilityTagOfDeclaration exposes the declaration-level `@stability` parser
// so the existing usage diagnostics reuse one implementation.
func StabilityTagOfDeclaration(declaration *ast.Node) string {
	return stabilityOfDeclarationTag(declaration)
}

// InternalTagOfDeclaration reports whether a declaration carries the parsed
// JSDoc `@internal` tag. Native TypeScript recognizes the same tag for
// `stripInternal` declaration emit: a tagged declaration is omitted from the
// emitted declarations, so it is not part of the public API. Only parsed tags
// match, so prose mentioning the word is never treated as a tag. A variable's
// JSDoc is usually attached to its enclosing variable statement, so that is
// checked too, mirroring the `@stability` parser.
func InternalTagOfDeclaration(declaration *ast.Node) bool {
	if declaration == nil {
		return false
	}
	nodes := []*ast.Node{declaration}
	if declaration.Parent != nil && declaration.Parent.Kind == ast.KindVariableDeclarationList &&
		declaration.Parent.Parent != nil && declaration.Parent.Parent.Kind == ast.KindVariableStatement {
		nodes = append(nodes, declaration.Parent.Parent)
	}
	for _, node := range nodes {
		if node.Flags&ast.NodeFlagsHasJSDoc == 0 {
			continue
		}
		for _, doc := range node.JSDoc(nil) {
			jsdoc := doc.AsJSDoc()
			if jsdoc == nil || jsdoc.Tags == nil {
				continue
			}
			for _, tag := range jsdoc.Tags.Nodes {
				if tag.Kind == ast.KindJSDocUnknownTag && tag.TagName().Text() == "internal" {
					return true
				}
			}
		}
	}
	return false
}

// DeclaredApiStability returns the stability tier declared for a symbol. The
// result is cached per checker on EffectLinks so runs over different source
// files share the same lookup. Declared lookups never trigger any computed
// surface analysis.
func (tp *TypeParser) DeclaredApiStability(symbol *ast.Symbol) ApiStabilityLevel {
	return tp.DeclaredApiStabilityOfSymbol(symbol).Level
}

// DeclaredApiStabilityOfSymbol returns the declared stability of a symbol,
// resolving import/export aliases and honouring a tag on the enclosing export
// declaration. Untagged symbols are stable. The result is cached per checker.
func (tp *TypeParser) DeclaredApiStabilityOfSymbol(symbol *ast.Symbol) ApiStabilityDeclaration {
	if tp == nil || symbol == nil {
		return ApiStabilityDeclaration{}
	}
	return Cached(&tp.links.ApiStabilityDeclaredSymbol, symbol, func() ApiStabilityDeclaration {
		return declaredStabilityOfSymbolChain(tp.checker, symbol)
	})
}

// DeclaredApiStabilityOfSignature returns the declared stability carried by a
// selected signature declaration. It is cached per checker on the raw
// (uninstantiated) signature so different overloads keep distinct tags; a
// symbol-level tag must never collapse them. An untagged signature is stable
// with no declaration, which lets callers fall back to the symbol.
func (tp *TypeParser) DeclaredApiStabilityOfSignature(signature *checker.Signature) ApiStabilityDeclaration {
	if tp == nil || signature == nil {
		return ApiStabilityDeclaration{}
	}
	raw := rawSignature(signature)
	if raw == nil {
		return ApiStabilityDeclaration{}
	}
	return Cached(&tp.links.ApiStabilityDeclaredSignature, raw, func() ApiStabilityDeclaration {
		return tp.DeclaredApiStabilityOfDeclaration(raw.Declaration())
	})
}

// DeclaredApiStabilityOfDeclaration returns the stability tag attached to a
// declaration and the declaration that carries it. The result is cached per
// checker; this is the shared declaration-level lookup used by the symbol and
// signature accessors.
func (tp *TypeParser) DeclaredApiStabilityOfDeclaration(declaration *ast.Node) ApiStabilityDeclaration {
	if tp == nil || declaration == nil {
		return ApiStabilityDeclaration{}
	}
	return Cached(&tp.links.ApiStabilityDeclaredDeclaration, declaration, func() ApiStabilityDeclaration {
		if stability := stabilityOfDeclarationTag(declaration); stability != "" {
			return ApiStabilityDeclaration{Level: apiStabilityRank(stability), Declaration: declaration}
		}
		return ApiStabilityDeclaration{}
	})
}

func apiStabilityRank(stability string) ApiStabilityLevel {
	switch stability {
	case "unstable":
		return ApiStabilityUnstable
	case "experimental":
		return ApiStabilityExperimental
	default:
		return ApiStabilityStable
	}
}

// declaredStabilityOfSymbolChain mirrors the reference resolution used by the
// stability usage rules: a declaration's own tag wins, then import/export
// aliases are followed. Alias cycles are broken by symbol identity rather than
// a fixed hop count, so long re-export chains keep their inherited stability.
func declaredStabilityOfSymbolChain(c *checker.Checker, symbol *ast.Symbol) ApiStabilityDeclaration {
	seen := make(map[*ast.Symbol]bool)
	for symbol != nil {
		if seen[symbol] {
			break
		}
		seen[symbol] = true
		for _, declaration := range symbol.Declarations {
			if stability := stabilityOfDeclarationTag(declaration); stability != "" {
				return ApiStabilityDeclaration{Level: apiStabilityRank(stability), Declaration: declaration}
			}
		}
		if symbol.Flags&ast.SymbolFlagsAlias == 0 {
			break
		}
		// The checker synthesizes a declaration-less `default` alias for `export =`
		// and JSON modules and panics when asked for its immediate target.
		if !hasAliasDeclaration(symbol) {
			break
		}
		next := c.GetImmediateAliasedSymbol(symbol)
		if next == symbol {
			break
		}
		symbol = next
	}
	return ApiStabilityDeclaration{}
}

func hasAliasDeclaration(symbol *ast.Symbol) bool {
	return slices.ContainsFunc(symbol.Declarations, ast.IsAliasSymbolDeclaration)
}

// ApiStabilitySession is the analysis-local memo for the computed stability of
// one export. The rule creates one session per export and queries the export's
// symbol (and any namespace member) through it, so repeated components are
// computed once within the export. After settling, the session publishes every
// complete, context-free concrete type and signature surface to the
// per-checker shared caches, so a later export, file or TypeParser over the
// same checker composes that snapshot instead of recomputing; the session's
// symbol and carrier memos, and every incomplete or blocked result, are
// discarded with the session. Declared stability is read from the per-checker
// declared caches and never needs a session.
type ApiStabilitySession struct {
	analysis *apiStabilityAnalysis
}

// NewApiStabilitySession starts a fresh computed-stability analysis. The
// session owns its symbol, signature, concrete type/component and substitution
// carrier memos; only the complete context-free surfaces published through
// settled snapshots outlive it.
func (tp *TypeParser) NewApiStabilitySession() *ApiStabilitySession {
	if tp == nil {
		return nil
	}
	return &ApiStabilitySession{analysis: newApiStabilityAnalysis(tp)}
}

// UsedBySymbol computes the stability the children of a symbol's public
// surface expose inside this session and settles cycle cuts: the root symbol's
// own declared tag is excluded from the returned dependencies and minimum. The
// own tag is exactly what DeclaredApiStabilityOfSymbol reports, read from the
// per-checker declared cache; a tag a function's declaration carries is
// recorded as a signature finding and is excluded together with the root
// symbol finding, while a tagged overload of another declaration stays an
// exposed child. Excluding the root tag is a result conversion only: the
// stored surface always keeps it, so the same symbol reached as a child of
// another root contributes its own tag to that parent. A complete result is
// memoized for later queries of the same session; a result cut by an
// unavailable component stays incomplete and is retried by a later query or
// session.
func (s *ApiStabilitySession) UsedBySymbol(symbol *ast.Symbol) ApiStabilityUsed {
	if s == nil || s.analysis == nil || symbol == nil {
		return ApiStabilityUsed{}
	}
	s.analysis.symbolSurface(symbol)
	s.analysis.settle()
	return apiStabilityUsedOfOwn(s.analysis.sessionSymbols[symbol], s.analysis.symbolOwnTag(symbol))
}

// UsedByType computes the stability the children of an already represented
// checker type expose inside this session. The type's own declared tag is
// excluded from the returned dependencies and minimum: a type alias
// application's own tag is its alias symbol's, and a plain represented type's
// own tag is its own symbol's. An underlying named type an alias resolves to
// is an exposed child and keeps contributing. The stored surface is untouched,
// so the same type reached as a child of another root still contributes its
// own tag to that parent.
func (s *ApiStabilitySession) UsedByType(t *checker.Type) ApiStabilityUsed {
	if s == nil || s.analysis == nil || t == nil {
		return ApiStabilityUsed{}
	}
	s.analysis.typeSurface(t, apiStabilityExpand, apiStabilitySubstitution{})
	s.analysis.settle()
	key := apiStabilityTypeKey{t: t, inspect: apiStabilityExpand}
	return apiStabilityUsedOfOwn(s.analysis.sessionTypes[key], s.analysis.typeOwnTag(t))
}

// UsedBySignature computes the stability the children of an already
// represented signature expose inside this session. The signature's own
// declared tag is excluded from the returned dependencies and minimum; its
// parameters, type parameters and return keep contributing. The raw signature
// identity makes each overload's own tag distinct from every other overload's.
func (s *ApiStabilitySession) UsedBySignature(signature *checker.Signature) ApiStabilityUsed {
	if s == nil || s.analysis == nil || signature == nil {
		return ApiStabilityUsed{}
	}
	s.analysis.signatureSurface(signature, apiStabilitySubstitution{})
	s.analysis.settle()
	key := apiStabilitySignatureKey{signature: signature}
	return apiStabilityUsedOfOwn(s.analysis.sessionSignatures[key], s.analysis.signatureOwnTag(signature))
}

// ApiStabilityUsedBySymbol computes the stability the children of a symbol's
// public surface expose in a fresh one-call session. The root symbol's own
// declared tag is excluded; use DeclaredApiStabilityOfSymbol to read it. The
// traversal walks compiler represented type graphs: an exported interface's own
// and inherited members, index signatures, call/construct signatures, generic
// constraints and defaults, type arguments of represented references, and the
// alias symbols written in public annotations that the checker erases. A named
// dependency reached inside the surface stays shallow (its own public call
// signatures are still inspected), so arbitrary referenced member graphs are
// never expanded. A child with an explicit `@stability` tag is a boundary: its
// declared level contributes to the parent surface and the child's internals
// are not expanded, while the same component keeps being audited when it is
// itself the queried root. A child without an explicit tag composes within the
// current shallow and anonymous graph boundaries: its public call and construct
// signatures, inherited surface, index signatures and represented members are
// inspected under the same rules. Each call starts from a fresh analysis, but a
// complete, settled, context-free concrete type or signature surface is
// published to the per-checker shared caches and reused by a later export, file
// or TypeParser over the same checker; symbol-root results and every
// incomplete, blocked or substitution-context result stay analysis-local.
//
// Public components the checker has not computed are materialized through its
// ordinary lazy accessors, preferring an already computed result. Before a
// potentially recursive read the non-evaluating pre-flight recursion guard
// inspects the represented carrier and the raw declarations with the compiler's
// own relations; a read whose expansion may evaluate a recursive alias, or
// whose safety the guard cannot establish, is refused and leaves the result
// incomplete (blocked), which is never reported as stable and is retried by a
// later session. An inferred component (a function return or a variable
// initializer) is read through the checker's own lazy inference, which
// attributes the diagnostics it produces to the declaring file's expressions
// exactly as that file's ordinary check would, so the rule never suppresses or
// consumes compiler diagnostics and the checked program keeps its own types and
// diagnostics exactly.
func (tp *TypeParser) ApiStabilityUsedBySymbol(symbol *ast.Symbol) ApiStabilityUsed {
	return tp.NewApiStabilitySession().UsedBySymbol(symbol)
}

// ApiStabilityUsedByType computes the stability the children of an already
// represented checker type expose in a fresh one-call session. The type's own
// declared tag is excluded; use the declared accessors to read it. It switches
// on TypeFlags/ObjectFlags and reads represented inner fields; a reference is
// inspected through its concrete public surface when expanded and through its
// declaration plus represented type arguments when shallow. Recursive alias
// applications are never expanded.
func (tp *TypeParser) ApiStabilityUsedByType(t *checker.Type) ApiStabilityUsed {
	return tp.NewApiStabilitySession().UsedByType(t)
}

// ApiStabilityUsedBySignature computes the stability the children of an
// already represented signature expose in a fresh one-call session. The
// signature's own declared tag is excluded; use DeclaredApiStabilityOfSignature
// to read it. A concrete signature's instantiated parameter and return
// components are preferred; the uninstantiated target plus the represented
// substitution remains the fallback, so a directly exported inferred return
// such as `make<E>()` still exposes `E` without expanding a recursive alias.
func (tp *TypeParser) ApiStabilityUsedBySignature(signature *checker.Signature) ApiStabilityUsed {
	return tp.NewApiStabilitySession().UsedBySignature(signature)
}

// apiStabilityOwnTag identifies a root component's own declared tag inside a
// stored surface: the root symbol or raw signature identity together with the
// declaration that carries the root's declared stability. Stored surfaces
// always keep a component's own tag so the same component reached as a child
// contributes it to the parent surface; the result conversion for a root query
// removes exactly the findings this structure matches. The declaration identity
// also matches a tagged root signature: a function declaration's tag is
// recorded as a signature finding, and it is the root's own tag exactly when
// the declared accessor reports the same declaration.
type apiStabilityOwnTag struct {
	symbol      *ast.Symbol
	signature   *checker.Signature
	declaration *ast.Node
}

// excludes reports whether one stored finding is the root's own declared tag.
// A nil root field never matches, so a child-only conversion (a component, a
// nil owner) excludes nothing.
func (own apiStabilityOwnTag) excludes(key apiStabilityFindingKey, finding apiStabilityFinding) bool {
	if own.symbol != nil && key.signature == nil && key.symbol == own.symbol {
		return true
	}
	if own.signature != nil && key.signature != nil && rawSignature(key.signature) == rawSignature(own.signature) {
		return true
	}
	return own.declaration != nil && finding.declaration == own.declaration
}

// symbolOwnTag builds the own-tag identity of a symbol root from the declared
// accessor. The declared lookup only reads the per-checker declared caches, so
// building the identity never computes a surface.
func (a *apiStabilityAnalysis) symbolOwnTag(symbol *ast.Symbol) apiStabilityOwnTag {
	if symbol == nil {
		return apiStabilityOwnTag{}
	}
	declared := a.tp.DeclaredApiStabilityOfSymbol(symbol)
	return apiStabilityOwnTag{symbol: symbol, declaration: declared.Declaration}
}

// typeOwnTag builds the own-tag identity of a represented type root. A type
// alias application's own tag is the alias symbol's; an underlying named type
// the alias resolves to stays an exposed child. Any other represented type's
// own tag is its own symbol's.
func (a *apiStabilityAnalysis) typeOwnTag(t *checker.Type) apiStabilityOwnTag {
	if t == nil {
		return apiStabilityOwnTag{}
	}
	if alias := t.Alias(); alias != nil && alias.Symbol() != nil {
		return a.symbolOwnTag(alias.Symbol())
	}
	if symbol := t.Symbol(); symbol != nil {
		return a.symbolOwnTag(symbol)
	}
	return apiStabilityOwnTag{}
}

// signatureOwnTag builds the own-tag identity of a signature root. The raw
// (uninstantiated) signature keeps distinct overload tags distinct.
func (a *apiStabilityAnalysis) signatureOwnTag(signature *checker.Signature) apiStabilityOwnTag {
	raw := rawSignature(signature)
	if raw == nil {
		return apiStabilityOwnTag{}
	}
	declared := a.tp.DeclaredApiStabilityOfSignature(raw)
	return apiStabilityOwnTag{signature: raw, declaration: declared.Declaration}
}

// apiStabilityUsedOf converts a stored symbol surface into the public result
// for a symbol-rooted query, excluding the root symbol's own finding. It is the
// settlement-test entry point; the session accessors build the richer own-tag
// identity, which also excludes a root's tagged signature declaration.
func apiStabilityUsedOf(surface apiStabilitySurface, owner *ast.Symbol) ApiStabilityUsed {
	return apiStabilityUsedOfOwn(surface, apiStabilityOwnTag{symbol: owner})
}

// apiStabilityUsedOfOwn converts a stored surface into the public result,
// skipping every finding that is the root's own declared tag. The stored
// surface itself is never modified.
func apiStabilityUsedOfOwn(surface apiStabilitySurface, own apiStabilityOwnTag) ApiStabilityUsed {
	used := ApiStabilityUsed{Minimum: ApiStabilityStable, Incomplete: !surface.complete}
	for key, finding := range surface.findings {
		if own.excludes(key, finding) {
			continue
		}
		declaration := finding.declaration
		if declaration == nil {
			declaration = key.declaration
		}
		used.Dependencies = append(used.Dependencies, ApiStabilityDependency{
			Symbol:      key.symbol,
			Signature:   key.signature,
			Declaration: declaration,
			Level:       finding.level,
		})
		if finding.level > used.Minimum {
			used.Minimum = finding.level
		}
	}
	sortApiStabilityDependencies(used.Dependencies)
	return used
}

func sortApiStabilityDependencies(deps []ApiStabilityDependency) {
	sort.Slice(deps, func(i, j int) bool {
		left, right := deps[i], deps[j]
		leftName, rightName := apiStabilityDependencySortName(left), apiStabilityDependencySortName(right)
		if leftName != rightName {
			return leftName < rightName
		}
		if left.Level != right.Level {
			return left.Level > right.Level
		}
		return left.Symbol != nil && right.Symbol == nil
	})
}

func apiStabilityDependencySortName(dependency ApiStabilityDependency) string {
	if name := apiStabilityUsableSymbolName(dependency.Symbol); name != "" {
		return name
	}
	if name, ok := apiStabilityDeclarationDisplayName(dependency.Declaration); ok {
		return name
	}
	if name := apiStabilityLateBoundSymbolDisplayName(dependency.Symbol); name != "" {
		return name
	}
	if dependency.Symbol != nil && dependency.Symbol.Name != "" {
		return dependency.Symbol.Name
	}
	if dependency.Signature != nil {
		return "signature"
	}
	return "<anonymous>"
}

// ApiStabilityDependencyName returns the display name of an exposed offender
// for diagnostics: the symbol name, the declaration's safely read name, or a
// descriptive fallback for anonymous signatures. A computed declaration name is
// never evaluated; its display spelling comes from its declaration symbol or
// its well-known symbol expression.
func ApiStabilityDependencyName(dependency ApiStabilityDependency) string {
	if name := apiStabilityUsableSymbolName(dependency.Symbol); name != "" {
		return name
	}
	if name, ok := apiStabilityDeclarationDisplayName(dependency.Declaration); ok {
		return name
	}
	if name := apiStabilityLateBoundSymbolDisplayName(dependency.Symbol); name != "" {
		return name
	}
	if dependency.Symbol != nil && dependency.Symbol.Name != "" {
		return dependency.Symbol.Name
	}
	if dependency.Declaration != nil {
		switch dependency.Declaration.Kind {
		case ast.KindCallSignature:
			return "call signature"
		case ast.KindConstructSignature:
			return "constructor"
		case ast.KindIndexSignature:
			return "index signature"
		}
		if name := ast.GetNameOfDeclaration(dependency.Declaration); name != nil && name.Kind == ast.KindComputedPropertyName {
			return "[computed]"
		}
	}
	if dependency.Signature != nil {
		if dependency.Signature.Flags()&checker.SignatureFlagsConstruct != 0 {
			return "constructor"
		}
		return "call signature"
	}
	return "<anonymous>"
}

// apiStabilityUsableSymbolName returns a symbol name suitable for display,
// excluding the binder's `__computed` placeholder and the checker's `__@`
// late-bound member spellings. A computed declaration renders its own readable
// spelling instead of exposing those internal names.
func apiStabilityUsableSymbolName(symbol *ast.Symbol) string {
	if symbol == nil {
		return ""
	}
	name := symbol.Name
	if name == "" || strings.HasPrefix(name, ast.InternalSymbolNamePrefix) || strings.HasPrefix(name, "__@") {
		return ""
	}
	return name
}

// apiStabilityLateBoundSymbolDisplayName renders the bound property name of a
// checker late-bound member (`__@name@id`) for diagnostics and sorting. The
// checker records that name when it late-binds a computed declaration, so
// reading it never evaluates the source expression and never leaks the internal
// spelling. It returns "" for anything that is not a late-bound symbol name.
func apiStabilityLateBoundSymbolDisplayName(symbol *ast.Symbol) string {
	if symbol == nil || !checker.IsKnownSymbol(symbol) {
		return ""
	}
	name := strings.TrimPrefix(symbol.Name, ast.InternalSymbolNamePrefix+"@")
	if index := strings.LastIndex(name, "@"); index >= 0 {
		name = name[:index]
	}
	if name == "" {
		return ""
	}
	return "[" + name + "]"
}

// apiStabilityDeclarationDisplayName returns the name of a declaration for
// sorting and display without evaluating the name. A simply named declaration
// keeps its source spelling. A computed name is never asked for its text: a
// well-known `Symbol.<name>` expression is rendered from its own identifiers, a
// symbol the binder already attached to the declaration names it, and any
// other computed name reports no name so the caller can fall back
// deterministically. The boolean reports whether a name was available.
func apiStabilityDeclarationDisplayName(declaration *ast.Node) (string, bool) {
	if declaration == nil {
		return "", false
	}
	name := ast.GetNameOfDeclaration(declaration)
	if name == nil {
		return "", false
	}
	if simple, ok := apiStabilitySimpleNameText(name); ok {
		return simple, true
	}
	if name.Kind == ast.KindComputedPropertyName {
		if wellKnown := apiStabilityWellKnownSymbolDisplayName(name.Expression()); wellKnown != "" {
			return wellKnown, true
		}
		// A literal computed name denotes exactly the literal it is written
		// with; reading that literal is not an evaluation. An identifier
		// computed name denotes the identifier's value, not its spelling, so
		// it is never rendered from source text.
		if expression := name.Expression(); expression != nil {
			switch expression.Kind {
			case ast.KindStringLiteral, ast.KindNumericLiteral, ast.KindBigIntLiteral, ast.KindNoSubstitutionTemplateLiteral:
				return expression.Text(), true
			}
		}
		if declarationSymbol := declaration.Symbol(); declarationSymbol != nil {
			symbolName := declarationSymbol.Name
			if symbolName != "" && !strings.HasPrefix(symbolName, ast.InternalSymbolNamePrefix) {
				return symbolName, true
			}
		}
	}
	return "", false
}

// apiStabilitySimpleNameText returns the source text of a declaration name that
// has one without evaluation. A computed or otherwise dynamic name reports
// false; its text is never read.
func apiStabilitySimpleNameText(name *ast.Node) (string, bool) {
	if name == nil {
		return "", false
	}
	switch name.Kind {
	case ast.KindIdentifier, ast.KindStringLiteral, ast.KindNumericLiteral, ast.KindBigIntLiteral, ast.KindPrivateIdentifier:
		return name.Text(), true
	}
	return "", false
}

// apiStabilityAnalysis carries the cycle guards and memo for one computation.
// Complete symbol, signature, type and component results are reused inside the
// analysis, results contaminated by a cycle are settled with a bounded
// fixpoint, and a result whose components are unavailable stays incomplete and
// is never treated as stable. After the fixpoint has converged, every complete,
// settled, context-free concrete type and signature result is also published
// to the per-checker shared caches as a root-excluding snapshot; a result that
// is still in progress, cycle-cut before promotion, blocked or otherwise
// incomplete is never published, so a partial result can never leak into
// another export. Symbol surfaces stay analysis-local: a symbol root composes
// its own declared identity with the reusable concrete type and signature
// surfaces.
type apiStabilityAnalysis struct {
	tp *TypeParser

	sessionSymbols    map[*ast.Symbol]apiStabilitySurface
	sessionTypes      map[apiStabilityTypeKey]apiStabilitySurface
	sessionComponents map[apiStabilityTypeKey]apiStabilitySurface
	sessionSignatures map[apiStabilitySignatureKey]apiStabilitySurface
	typeFrames        map[apiStabilityTypeKey]*apiStabilitySubstFrame
	signatureFrames   map[apiStabilitySignatureKey]*apiStabilitySubstFrame

	// carriers interns the concrete enclosing identity of raw-component
	// traversals for this analysis only. Equal chains share one node pointer so
	// cache and cycle keys compare by concrete identity instead of a serialized
	// substitution context; the table is discarded with the analysis.
	carriers map[apiStabilityCarrier]*apiStabilityCarrier

	// settleSymbols, settleTypes, settleComponents and settleSignatures record
	// the first-insertion order of the session results. Settlement recollects
	// incomplete results in that order instead of Go's randomized map order, so
	// the bounded fixpoint visits exactly the same results in the same sequence
	// over the same checker state and the reported findings are reproducible.
	// The order is analysis-local bookkeeping; it is never part of any cache key.
	settleSymbols    []*ast.Symbol
	settleTypes      []apiStabilityTypeKey
	settleComponents []apiStabilityTypeKey
	settleSignatures []apiStabilitySignatureKey

	activeSymbols      map[apiStabilitySymbolKey]bool
	activeTypes        map[apiStabilityTypeKey]bool
	activeComponents   map[apiStabilityTypeKey]bool
	activeSignatures   map[apiStabilitySignatureKey]bool
	activeAliases      map[apiStabilityAliasKey]bool
	activeConditionals map[apiStabilityAliasKey]bool

	signatureArgs           map[*checker.Signature][]*checker.Type
	signatureParameterCache map[*checker.Signature]map[*checker.Type]*checker.Type

	// safetySafeMemo records completed Safe guard verdicts for this analysis,
	// keyed by the concrete read identity (operation, represented identity and
	// interned substitution carrier). Only Safe is ever recorded: Unknown,
	// recursion and exhaustion stay retryable by later scans and analyses.
	safetySafeMemo map[apiStabilitySafetyVerdictKey]struct{}

	// typeNameSymbols caches successful symbol lookups for raw type-name,
	// heritage, qualifier and declaration-name nodes by AST identity. A symbol
	// lookup at a node is a pure declaration/scope lookup that does not depend
	// on type bindings; no resolved type is ever cached here.
	typeNameSymbols map[*ast.Node]*ast.Symbol

	work       int
	safetyWork int

	// materializationReads records the lazy reads whose checker accessor already
	// advanced the materialization epoch in this analysis, keyed by the read
	// operation and its concrete symbol or annotation node. Re-reading the same
	// component cannot materialize anything new (the checker caches it, or the
	// call is a declaration-only lookup), so the epoch only advances once per
	// identity. Without this, a repeated refused or cached lookup would keep
	// every blocked result stale and settlement would recollect surfaces that
	// provably cannot advance.
	materializationReads map[apiStabilityMaterializationReadKey]struct{}

	// materializationEpoch advances whenever this analysis performs a lazy
	// checker read that can materialize represented components (a guarded read
	// that the pre-flight guard authorized, an inferred read the checker's own
	// check lifecycle allows, or a nested read a guard verification performed)
	// and whenever a settle round changes any surface. A blocked surface that
	// was last collected before the latest advancement may have become
	// collectable or may have a newly changed represented dependency; a surface
	// collected after it provably observed the current state.
	materializationEpoch int
}

const (
	// apiStabilityMaxWork bounds one analysis. The guard exists only so a
	// pathological represented graph cannot run away; ordinary surfaces with
	// deeply nested explicit components stay far below it. Exhausting the bound
	// marks the result incomplete, which keeps it out of every cache.
	apiStabilityMaxWork = 100_000
	// apiStabilityMaxSettleRounds bounds the cycle fixpoint. Each round can only
	// add findings, so convergence is monotone; the bound is a safety net.
	apiStabilityMaxSettleRounds = 64
	// apiStabilityMaxSettleSweeps bounds the extra rounds an exhausted analysis
	// runs after the fixpoint converges. A recollection in the final round may
	// itself have advanced the epoch, making earlier results stale; the extra
	// rounds recollect them and stop as soon as a round changes nothing. A
	// result they do not recover stays blocked, and the analysis discards it
	// with its session, so a later export or call retries it.
	apiStabilityMaxSettleSweeps = 4
)

func newApiStabilityAnalysis(tp *TypeParser) *apiStabilityAnalysis {
	return &apiStabilityAnalysis{
		tp:                      tp,
		sessionSymbols:          make(map[*ast.Symbol]apiStabilitySurface),
		sessionTypes:            make(map[apiStabilityTypeKey]apiStabilitySurface),
		sessionComponents:       make(map[apiStabilityTypeKey]apiStabilitySurface),
		sessionSignatures:       make(map[apiStabilitySignatureKey]apiStabilitySurface),
		typeFrames:              make(map[apiStabilityTypeKey]*apiStabilitySubstFrame),
		signatureFrames:         make(map[apiStabilitySignatureKey]*apiStabilitySubstFrame),
		carriers:                make(map[apiStabilityCarrier]*apiStabilityCarrier),
		activeSymbols:           make(map[apiStabilitySymbolKey]bool),
		activeTypes:             make(map[apiStabilityTypeKey]bool),
		activeComponents:        make(map[apiStabilityTypeKey]bool),
		activeSignatures:        make(map[apiStabilitySignatureKey]bool),
		activeAliases:           make(map[apiStabilityAliasKey]bool),
		activeConditionals:      make(map[apiStabilityAliasKey]bool),
		signatureArgs:           make(map[*checker.Signature][]*checker.Type),
		signatureParameterCache: make(map[*checker.Signature]map[*checker.Type]*checker.Type),
	}
}

func (a *apiStabilityAnalysis) consumeWork() bool {
	a.work++
	return a.work <= apiStabilityMaxWork
}

// settle resolves cycle cuts with a bounded monotone fixpoint, then marks
// every result that was established complete as settled within this analysis.
// Findings are recomputed until they stop changing; a recomputed surface is
// unioned into the stored one so a dependency that is itself still settling can
// never erase a finding an earlier round established. Results whose only
// incompleteness came from cycle back-edges are then established complete,
// because a back-edge's own direct findings are collected when the cut target
// is first entered. Results that remain blocked (unavailable components or an
// unevaluable conditional outcome) are never promoted, so a partial result is
// never reported as stable; a later export or call retries from a new session.
//
// After the fixpoint converges, an exhausted analysis runs bounded extra
// rounds. A recollection in the final round may itself have materialized a
// component or advanced the epoch, making results collected earlier stale
// again; the extra rounds recollect exactly those stale results. A blocked
// result can only gain findings this way; it is never promoted. A result the
// extra rounds do not recover stays blocked, and the analysis discards it with
// its session, so a later export or call retries it.
//
// Once settling is done, every complete, context-free type and signature
// result is published to the per-checker shared caches. A result that is still
// in progress, blocked or cycle-cut before promotion is never published; a
// promoted cycle result is published only here, after the fixpoint established
// that its findings no longer change.
func (a *apiStabilityAnalysis) settle() {
	if a.hasIncomplete() {
		converged := false
		for range apiStabilityMaxSettleRounds {
			if !a.settleRound() {
				converged = true
				break
			}
			// A change to any surface can propagate into a result that was
			// collected earlier; advancing the epoch makes exactly those
			// earlier results stale, while a result collected after the change
			// has already observed it.
			a.materializationEpoch++
		}
		if converged {
			if a.safetyBudgetExhausted() {
				for range apiStabilityMaxSettleSweeps {
					if !a.settleRound() {
						break
					}
					// A recovery round can change a surface exactly like a
					// fixpoint round: earlier blocked parents may now be
					// stale, so they must be revisited within the bounded
					// sweeps. This is the same propagation the fixpoint loop
					// performs; it authorizes no additional guard read.
					a.materializationEpoch++
				}
			}
			a.promoteSettled()
		}
	}
	a.publishCompleteSurfaces()
}

// storeSymbolSurface records a symbol result and its first-insertion order.
func (a *apiStabilityAnalysis) storeSymbolSurface(symbol *ast.Symbol, surface apiStabilitySurface) {
	if _, exists := a.sessionSymbols[symbol]; !exists {
		a.settleSymbols = append(a.settleSymbols, symbol)
	}
	a.sessionSymbols[symbol] = surface
}

// storeTypeSurface records a type result and its first-insertion order.
func (a *apiStabilityAnalysis) storeTypeSurface(key apiStabilityTypeKey, surface apiStabilitySurface) {
	if _, exists := a.sessionTypes[key]; !exists {
		a.settleTypes = append(a.settleTypes, key)
	}
	a.sessionTypes[key] = surface
}

// storeComponentSurface records an object-component result and its
// first-insertion order.
func (a *apiStabilityAnalysis) storeComponentSurface(key apiStabilityTypeKey, surface apiStabilitySurface) {
	if _, exists := a.sessionComponents[key]; !exists {
		a.settleComponents = append(a.settleComponents, key)
	}
	a.sessionComponents[key] = surface
}

// storeSignatureSurface records a signature result and its first-insertion
// order.
func (a *apiStabilityAnalysis) storeSignatureSurface(key apiStabilitySignatureKey, surface apiStabilitySurface) {
	if _, exists := a.sessionSignatures[key]; !exists {
		a.settleSignatures = append(a.settleSignatures, key)
	}
	a.sessionSignatures[key] = surface
}

// restoreOwnTagFinding re-adds one finding a published snapshot removed as the
// component's own declared tag. The declared level and provenance come from
// the per-checker declared caches — the snapshot stores only the concrete
// identity — so composition is always "declared separately". The removed key
// was produced by exactly one of the three record helpers, which re-derives
// the same level and declaration from the same caches.
func (a *apiStabilityAnalysis) restoreOwnTagFinding(surface *apiStabilitySurface, key apiStabilityFindingKey) {
	switch {
	case key.signature != nil:
		a.recordSignatureStability(surface, key.signature)
	case key.symbol != nil:
		a.recordSymbol(surface, key.symbol)
	case key.declaration != nil:
		a.recordDeclarationStability(surface, key.declaration)
	}
}

// sharedTypeSurface returns a fresh analysis-local copy of the published
// complete surface of a concrete context-free type, re-adding the component's
// own declared-tag findings from the declared caches. It reports false when no
// entry exists, so the caller computes locally. The copy keeps the published
// snapshot immutable while the caller composes and mutates its own surface.
func (a *apiStabilityAnalysis) sharedTypeSurface(t *checker.Type, inspect apiStabilityInspection) (apiStabilitySurface, bool) {
	if a.tp == nil || a.tp.links == nil || t == nil {
		return apiStabilitySurface{}, false
	}
	entry := a.tp.links.ApiStabilitySurfaceType.TryGet(apiStabilitySurfaceTypeKey{t: t, inspect: inspect})
	if entry == nil {
		return apiStabilitySurface{}, false
	}
	surface := newApiStabilitySurface()
	maps.Copy(surface.findings, entry.findings)
	for _, key := range entry.removed {
		a.restoreOwnTagFinding(&surface, key)
	}
	return surface, true
}

// sharedSignatureSurface is sharedTypeSurface for a concrete signature
// identity.
func (a *apiStabilityAnalysis) sharedSignatureSurface(signature *checker.Signature) (apiStabilitySurface, bool) {
	if a.tp == nil || a.tp.links == nil || signature == nil {
		return apiStabilitySurface{}, false
	}
	entry := a.tp.links.ApiStabilitySurfaceSignature.TryGet(signature)
	if entry == nil {
		return apiStabilitySurface{}, false
	}
	surface := newApiStabilitySurface()
	maps.Copy(surface.findings, entry.findings)
	for _, key := range entry.removed {
		a.restoreOwnTagFinding(&surface, key)
	}
	return surface, true
}

// publishTypeSurface publishes one complete, settled, context-free type surface
// as an immutable root-excluding snapshot. Every finding that matches the
// component's own declared tag is removed and its concrete identity recorded;
// the declared level stays in the per-checker declared caches. An in-progress
// result is never stored; a cycle-cut result only reaches complete after the
// settle fixpoint promoted it; a blocked or otherwise incomplete result is
// never published. The first entry for a key wins: complete recomputations over
// the same checker state agree, so overwriting could not add information and
// the snapshot stays deterministic.
func (a *apiStabilityAnalysis) publishTypeSurface(t *checker.Type, inspect apiStabilityInspection, surface apiStabilitySurface) {
	if a.tp == nil || a.tp.links == nil || t == nil || !surface.complete || surface.cutCycle || surface.blocked {
		return
	}
	store := &a.tp.links.ApiStabilitySurfaceType
	key := apiStabilitySurfaceTypeKey{t: t, inspect: inspect}
	if store.TryGet(key) != nil {
		return
	}
	own := a.typeOwnTag(t)
	entry := apiStabilitySharedSurface{findings: make(map[apiStabilityFindingKey]apiStabilityFinding, len(surface.findings))}
	for findingKey, finding := range surface.findings {
		if own.excludes(findingKey, finding) {
			entry.removed = append(entry.removed, findingKey)
			continue
		}
		entry.findings[findingKey] = finding
	}
	*store.Get(key) = entry
}

// publishSignatureSurface publishes one complete, settled, context-free
// signature surface with the signature's own declared tag removed exactly like
// publishTypeSurface does for a type.
func (a *apiStabilityAnalysis) publishSignatureSurface(signature *checker.Signature, surface apiStabilitySurface) {
	if a.tp == nil || a.tp.links == nil || signature == nil || !surface.complete || surface.cutCycle || surface.blocked {
		return
	}
	store := &a.tp.links.ApiStabilitySurfaceSignature
	if store.TryGet(signature) != nil {
		return
	}
	own := a.signatureOwnTag(signature)
	entry := apiStabilitySharedSurface{findings: make(map[apiStabilityFindingKey]apiStabilityFinding, len(surface.findings))}
	for findingKey, finding := range surface.findings {
		if own.excludes(findingKey, finding) {
			entry.removed = append(entry.removed, findingKey)
			continue
		}
		entry.findings[findingKey] = finding
	}
	*store.Get(signature) = entry
}

// publishCompleteSurfaces pushes every complete, settled, context-free session
// result into the per-checker shared caches. It runs after the settle fixpoint
// (including cycle promotion) so an in-progress or cycle-cut surface is never
// published. Results computed under a substitution carrier stay analysis-local.
// Symbol surfaces stay analysis-local too: a symbol root composes its own
// declared identity with the reusable concrete type and signature surfaces.
func (a *apiStabilityAnalysis) publishCompleteSurfaces() {
	if a.tp == nil || a.tp.links == nil {
		return
	}
	for key, surface := range a.sessionTypes {
		if key.carrier == nil {
			a.publishTypeSurface(key.t, key.inspect, surface)
		}
	}
	for key, surface := range a.sessionSignatures {
		if key.carrier == nil {
			a.publishSignatureSurface(key.signature, surface)
		}
	}
}

// apiStabilityNodeVisitKey renders a stable visit key for a declaration node.
func apiStabilityNodeVisitKey(node *ast.Node) string {
	if node == nil {
		return ""
	}
	file := ""
	if sourceFile := ast.GetSourceFileOfNode(node); sourceFile != nil {
		file = string(sourceFile.FileName())
	}
	return file + "#" + strconv.Itoa(node.Pos())
}

// apiStabilitySymbolVisitKey renders a stable visit key for a symbol result.
func apiStabilitySymbolVisitKey(symbol *ast.Symbol) string {
	if symbol == nil {
		return ""
	}
	for _, declaration := range symbol.Declarations {
		if declaration != nil {
			return symbol.Name + "@" + apiStabilityNodeVisitKey(declaration)
		}
	}
	return symbol.Name
}

// apiStabilityTypeVisitKey renders a best-effort stable visit key for a
// represented type result. It only orders the rare results that settlement had
// to reconcile because they were stored without the store helpers.
func apiStabilityTypeVisitKey(key apiStabilityTypeKey) string {
	name := ""
	if key.t != nil {
		if symbol := key.t.Symbol(); symbol != nil {
			name = symbol.Name
		}
		if name == "" {
			if alias := key.t.Alias(); alias != nil && alias.Symbol() != nil {
				name = alias.Symbol().Name
			}
		}
		if name == "" && key.t.ObjectFlags()&checker.ObjectFlagsReference != 0 && key.t.Target() != nil {
			if symbol := key.t.Target().Symbol(); symbol != nil {
				name = symbol.Name
			}
		}
	}
	return name + "@" + strconv.Itoa(int(key.inspect))
}

// apiStabilitySignatureVisitKey renders a stable visit key for a signature
// result.
func apiStabilitySignatureVisitKey(key apiStabilitySignatureKey) string {
	if key.signature == nil {
		return ""
	}
	raw := rawSignature(key.signature)
	if raw == nil {
		return ""
	}
	return apiStabilityNodeVisitKey(raw.Declaration())
}

// reconcileSettleOrder enrolls session results that were stored without the
// store helpers, so settlement still visits them. Production results always go
// through the helpers, whose size fast path keeps the order slices in sync; an
// analysis-local seeded result (a test that installs a cycle cut directly) is
// appended here in a canonical order so the visit sequence stays
// deterministic.
func (a *apiStabilityAnalysis) reconcileSettleOrder() {
	if len(a.sessionSymbols) != len(a.settleSymbols) {
		seen := make(map[*ast.Symbol]struct{}, len(a.settleSymbols))
		for _, symbol := range a.settleSymbols {
			seen[symbol] = struct{}{}
		}
		missing := make([]*ast.Symbol, 0, len(a.sessionSymbols)-len(a.settleSymbols))
		for symbol := range a.sessionSymbols {
			if _, ok := seen[symbol]; !ok {
				missing = append(missing, symbol)
			}
		}
		sort.Slice(missing, func(i, j int) bool {
			return apiStabilitySymbolVisitKey(missing[i]) < apiStabilitySymbolVisitKey(missing[j])
		})
		a.settleSymbols = append(a.settleSymbols, missing...)
	}
	if len(a.sessionTypes) != len(a.settleTypes) {
		seen := make(map[apiStabilityTypeKey]struct{}, len(a.settleTypes))
		for _, key := range a.settleTypes {
			seen[key] = struct{}{}
		}
		missing := make([]apiStabilityTypeKey, 0, len(a.sessionTypes)-len(a.settleTypes))
		for key := range a.sessionTypes {
			if _, ok := seen[key]; !ok {
				missing = append(missing, key)
			}
		}
		sort.Slice(missing, func(i, j int) bool {
			return apiStabilityTypeVisitKey(missing[i]) < apiStabilityTypeVisitKey(missing[j])
		})
		a.settleTypes = append(a.settleTypes, missing...)
	}
	if len(a.sessionComponents) != len(a.settleComponents) {
		seen := make(map[apiStabilityTypeKey]struct{}, len(a.settleComponents))
		for _, key := range a.settleComponents {
			seen[key] = struct{}{}
		}
		missing := make([]apiStabilityTypeKey, 0, len(a.sessionComponents)-len(a.settleComponents))
		for key := range a.sessionComponents {
			if _, ok := seen[key]; !ok {
				missing = append(missing, key)
			}
		}
		sort.Slice(missing, func(i, j int) bool {
			return apiStabilityTypeVisitKey(missing[i]) < apiStabilityTypeVisitKey(missing[j])
		})
		a.settleComponents = append(a.settleComponents, missing...)
	}
	if len(a.sessionSignatures) != len(a.settleSignatures) {
		seen := make(map[apiStabilitySignatureKey]struct{}, len(a.settleSignatures))
		for _, key := range a.settleSignatures {
			seen[key] = struct{}{}
		}
		missing := make([]apiStabilitySignatureKey, 0, len(a.sessionSignatures)-len(a.settleSignatures))
		for key := range a.sessionSignatures {
			if _, ok := seen[key]; !ok {
				missing = append(missing, key)
			}
		}
		sort.Slice(missing, func(i, j int) bool {
			return apiStabilitySignatureVisitKey(missing[i]) < apiStabilitySignatureVisitKey(missing[j])
		})
		a.settleSignatures = append(a.settleSignatures, missing...)
	}
}

// settleRound recollects every incomplete surface once and reports whether any
// surface changed. Complete results are always skipped: a completed result is
// never recomputed. A blocked result is skipped only when it was already
// collected at the current advancement epoch, so a materializing read or a
// surface change that happened after its last collection is always revisited.
// Results are visited in their first-insertion order so the bounded fixpoint
// spends its rounds deterministically; a recollection that adds a new result
// appends it to the end of the order and is visited in the same round, and a
// result stored without the store helpers is reconciled into the order before
// the round.
func (a *apiStabilityAnalysis) settleRound() bool {
	a.reconcileSettleOrder()
	changed := false
	// The order slices can grow while a round runs: recollecting a result may
	// store new results through the store helpers. The dynamic bound visits
	// them in the same round, which the range-over-int rewrite would not.
	for index := 0; index < len(a.settleTypes); index++ { //nolint:intrange // bounds grow during the round
		key := a.settleTypes[index]
		surface := a.sessionTypes[key]
		if surface.complete || a.settleCannotAdvance(surface) {
			continue
		}
		startEpoch := a.materializationEpoch
		frame := a.typeFrames[key]
		recomputed := a.collectTypeSurface(key.t, key.inspect, apiStabilitySubstitution{frame: frame})
		merged := growApiStabilitySurface(surface, recomputed)
		if !merged.equals(surface) {
			changed = true
		}
		merged.observedEpoch = startEpoch
		a.sessionTypes[key] = merged
	}
	for index := 0; index < len(a.settleComponents); index++ { //nolint:intrange // bounds grow during the round
		key := a.settleComponents[index]
		surface := a.sessionComponents[key]
		if surface.complete || a.settleCannotAdvance(surface) {
			continue
		}
		startEpoch := a.materializationEpoch
		frame := a.typeFrames[key]
		recomputed := a.collectObjectComponentSurface(key.t, key.inspect, apiStabilitySubstitution{frame: frame})
		merged := growApiStabilitySurface(surface, recomputed)
		if !merged.equals(surface) {
			changed = true
		}
		merged.observedEpoch = startEpoch
		a.sessionComponents[key] = merged
	}
	for index := 0; index < len(a.settleSignatures); index++ { //nolint:intrange // bounds grow during the round
		key := a.settleSignatures[index]
		surface := a.sessionSignatures[key]
		if surface.complete || a.settleCannotAdvance(surface) {
			continue
		}
		startEpoch := a.materializationEpoch
		frame := a.signatureFrames[key]
		recomputed := a.collectSignatureSurface(rawSignature(key.signature), key.signature, apiStabilitySubstitution{frame: frame})
		merged := growApiStabilitySurface(surface, recomputed)
		if !merged.equals(surface) {
			changed = true
		}
		merged.observedEpoch = startEpoch
		a.sessionSignatures[key] = merged
	}
	for index := 0; index < len(a.settleSymbols); index++ { //nolint:intrange // bounds grow during the round
		symbol := a.settleSymbols[index]
		surface := a.sessionSymbols[symbol]
		if surface.complete || a.settleCannotAdvance(surface) {
			continue
		}
		startEpoch := a.materializationEpoch
		recomputed := a.collectSymbolSurface(symbol)
		merged := growApiStabilitySurface(surface, recomputed)
		if !merged.equals(surface) {
			changed = true
		}
		merged.observedEpoch = startEpoch
		a.sessionSymbols[symbol] = merged
	}
	return changed
}

// settleCannotAdvance reports whether a fixpoint recollection of an incomplete
// surface provably cannot add anything. A blocked surface is only skipped once
// the guard budget is exhausted and the surface was last collected at the
// current advancement epoch: no later analysis read could have materialized one
// of its components since, and no later surface change could have propagated a
// represented dependency finding into it. The surface stays blocked and is
// never promoted; a later export or call retries it after the checker
// materializes more of the program; an exhausted analysis also runs bounded
// extra rounds after the fixpoint converges, so a result that a final
// recollection made stale is revisited. Cycle-cut surfaces always keep
// recomputing: represented dependencies can still propagate even while the
// guard refuses new reads.
func (a *apiStabilityAnalysis) settleCannotAdvance(surface apiStabilitySurface) bool {
	if !surface.blocked || !a.safetyBudgetExhausted() {
		return false
	}
	return surface.observedEpoch == a.materializationEpoch
}

// noteMaterializingRead records that the analysis is about to perform a lazy
// checker read that can materialize represented components. Settlement uses the
// resulting epoch to decide whether a blocked result may have become
// collectable without authorizing any new guard read.
func (a *apiStabilityAnalysis) noteMaterializingRead() {
	a.materializationEpoch++
}

// noteMaterializingSymbolRead advances the materialization epoch for the first
// lazy read of one symbol operation and is a no-op for repeats: a repeat cannot
// materialize anything new, and letting it keep the epoch moving would mark
// every blocked result stale forever.
func (a *apiStabilityAnalysis) noteMaterializingSymbolRead(kind apiStabilityMaterializationReadKind, symbol *ast.Symbol) {
	key := apiStabilityMaterializationReadKey{kind: kind, symbol: symbol}
	if _, ok := a.materializationReads[key]; ok {
		return
	}
	if a.materializationReads == nil {
		a.materializationReads = make(map[apiStabilityMaterializationReadKey]struct{})
	}
	a.materializationReads[key] = struct{}{}
	a.noteMaterializingRead()
}

// noteMaterializingNodeRead advances the materialization epoch for the first
// lazy read of one annotation node and is a no-op for repeats.
func (a *apiStabilityAnalysis) noteMaterializingNodeRead(node *ast.Node) {
	key := apiStabilityMaterializationReadKey{kind: apiStabilityMaterializationReadNodeType, node: node}
	if _, ok := a.materializationReads[key]; ok {
		return
	}
	if a.materializationReads == nil {
		a.materializationReads = make(map[apiStabilityMaterializationReadKey]struct{})
	}
	a.materializationReads[key] = struct{}{}
	a.noteMaterializingRead()
}

// materializedMembersOfSymbol resolves a symbol's member table through the
// checker's ordinary lazy accessor after the caller established that the read
// is safe. The read is recorded as materializing.
func (a *apiStabilityAnalysis) materializedMembersOfSymbol(c *checker.Checker, symbol *ast.Symbol) ast.SymbolTable {
	a.noteMaterializingSymbolRead(apiStabilityMaterializationReadMembers, symbol)
	return checker.Checker_getMembersOfSymbol(c, symbol)
}

// materializedSignaturesOfSymbol resolves a signature member's signatures
// through the checker's ordinary lazy accessor after the caller established
// that the read is safe. The read is recorded as materializing.
func (a *apiStabilityAnalysis) materializedSignaturesOfSymbol(c *checker.Checker, symbol *ast.Symbol) []*checker.Signature {
	a.noteMaterializingSymbolRead(apiStabilityMaterializationReadSignatures, symbol)
	return checker.Checker_getSignaturesOfSymbol(c, symbol)
}

// growApiStabilitySurface unions a recomputed surface into the previously
// stored one. Findings only grow across fixpoint rounds, so a result
// established in an earlier round is never lost when a later round observes a
// dependency that is itself still settling; incompleteness is latched until
// promotion, and a blocked result is never promoted.
func growApiStabilitySurface(previous, recomputed apiStabilitySurface) apiStabilitySurface {
	merged := previous
	merged.findings = make(map[apiStabilityFindingKey]apiStabilityFinding, len(previous.findings)+len(recomputed.findings))
	maps.Copy(merged.findings, previous.findings)
	for key, finding := range recomputed.findings {
		if existing, ok := merged.findings[key]; !ok || finding.level > existing.level {
			merged.findings[key] = finding
		}
	}
	if !recomputed.complete {
		merged.complete = false
		merged.cutCycle = merged.cutCycle || recomputed.cutCycle
		merged.blocked = merged.blocked || recomputed.blocked
	}
	return merged
}

// promoteSettled marks cycle-cut results as complete after the fixpoint
// converged. A blocked result is never promoted.
func (a *apiStabilityAnalysis) promoteSettled() {
	for symbol, surface := range a.sessionSymbols {
		if !surface.complete && !surface.blocked {
			surface.complete, surface.cutCycle = true, false
			a.sessionSymbols[symbol] = surface
		}
	}
	for key, surface := range a.sessionTypes {
		if !surface.complete && !surface.blocked {
			surface.complete, surface.cutCycle = true, false
			a.sessionTypes[key] = surface
		}
	}
	for key, surface := range a.sessionComponents {
		if !surface.complete && !surface.blocked {
			surface.complete, surface.cutCycle = true, false
			a.sessionComponents[key] = surface
		}
	}
	for key, surface := range a.sessionSignatures {
		if !surface.complete && !surface.blocked {
			surface.complete, surface.cutCycle = true, false
			a.sessionSignatures[key] = surface
		}
	}
}

func (a *apiStabilityAnalysis) hasIncomplete() bool {
	for _, surface := range a.sessionSymbols {
		if !surface.complete {
			return true
		}
	}
	for _, surface := range a.sessionTypes {
		if !surface.complete {
			return true
		}
	}
	for _, surface := range a.sessionComponents {
		if !surface.complete {
			return true
		}
	}
	for _, surface := range a.sessionSignatures {
		if !surface.complete {
			return true
		}
	}
	return false
}

// symbolSurface computes the dependency surface of an export symbol in this
// analysis. A complete result is reused by later queries of the same session; a
// result blocked by an unavailable component is kept, but never promoted.
func (a *apiStabilityAnalysis) symbolSurface(symbol *ast.Symbol) apiStabilitySurface {
	if symbol == nil {
		return newApiStabilitySurface()
	}
	if cached, ok := a.sessionSymbols[symbol]; ok {
		return cached
	}
	startEpoch := a.materializationEpoch
	surface := a.collectSymbolSurface(symbol)
	if surface.blocked {
		surface.observedEpoch = startEpoch
	}
	a.storeSymbolSurface(symbol, surface)
	return surface
}

// collectSymbolSurface selects the represented sources of a symbol's public
// surface. A value symbol's type, a class/interface/enum declared type and a
// type alias's declared type are read through the checker: an already computed
// result is preferred, and a public component the checker has not computed is
// materialized through the ordinary lazy accessors. Materializing one public
// type or signature does not recursively expand referenced types; shallow
// named references still use their declaration surface only. Declared member
// and signature `@stability` tags are metadata and are read from the
// declaration structure without resolving any type.
func (a *apiStabilityAnalysis) collectSymbolSurface(symbol *ast.Symbol) apiStabilitySurface {
	surface := newApiStabilitySurface()
	empty := apiStabilitySubstitution{}
	flags := symbol.Flags
	handled := false

	// A default (or `export =`) assignment of an inline expression has no alias
	// target. The expression's represented checker type is the public surface;
	// it is materialized lazily when the checker has not computed it yet,
	// unless the expression would expand a recursive alias.
	for _, declaration := range symbol.Declarations {
		if declaration == nil || declaration.Kind != ast.KindExportAssignment {
			continue
		}
		assignment := declaration.AsExportAssignment()
		if assignment == nil || assignment.Expression == nil {
			continue
		}
		represented := a.typeFromNodeSafely(assignment.Expression, empty)
		if represented != nil {
			surface.merge(a.typeSurface(represented, apiStabilityExpand, empty))
			handled = true
		}
	}

	switch {
	case handled:
	case flags&ast.SymbolFlagsTypeAlias != 0:
		if declared := a.declaredTypeOfSymbol(symbol); declared != nil {
			surface.merge(a.typeSurface(declared, apiStabilityExpand, empty))
			handled = true
		} else {
			handled = a.collectTypeAliasDeclarationSurface(&surface, symbol, empty)
		}
	case flags&ast.SymbolFlagsInterface != 0:
		// Building an interface's declared type is lazy and never instantiates
		// a generic application; members stay unresolved until a member type is
		// requested.
		if declared := a.declaredTypeOfSymbol(symbol); declared != nil {
			surface.merge(a.typeSurface(declared, apiStabilityExpand, empty))
			handled = true
		}
	case flags&(ast.SymbolFlagsClass|ast.SymbolFlagsEnum) != 0:
		if declared := a.declaredTypeOfSymbol(symbol); declared != nil {
			surface.merge(a.typeSurface(declared, apiStabilityExpand, empty))
			handled = true
		}
		if value := a.valueTypeOfSymbol(symbol); value != nil {
			surface.merge(a.typeSurface(value, apiStabilityExpand, empty))
		}
	default:
		if value := a.valueTypeOfSymbol(symbol); value != nil {
			surface.merge(a.typeSurface(value, apiStabilityExpand, empty))
			handled = true
		}
	}

	if !handled {
		for _, declaration := range symbol.Declarations {
			a.collectDeclaredTagSurface(&surface, a.declarationAnnotationNode(declaration))
		}
		surface.block()
	}

	// Erased alias provenance: pair each declaration's annotation nodes with
	// the represented type the checker recorded for its annotation. The walk
	// only records alias symbols that survive in the represented component.
	for _, declaration := range symbol.Declarations {
		if declaration == nil || declaration.FunctionLikeData() != nil {
			continue
		}
		a.collectDeclarationProvenance(&surface, declaration, a.declarationRepresentedType(symbol, declaration), empty)
	}
	return surface
}

// typeOfSymbolSafely returns a symbol's checker type, preferring an already
// computed result and otherwise materializing it through the ordinary lazy
// accessor when the pre-flight recursion guard establishes that the resolution
// is bounded. An inferred type (an initializer or a function body) is read
// through the checker's own lazy inference: diagnostics inference produces are
// attributed to the declaring file's expressions, exactly as that file's
// ordinary check would attribute them, so the read neither consumes nor
// duplicates them.
func (a *apiStabilityAnalysis) typeOfSymbolSafely(symbol *ast.Symbol) *checker.Type {
	if symbol == nil {
		return nil
	}
	if cached := a.tp.checker.GetResolvedTypeOfSymbolIfMaterialized(symbol); cached != nil {
		return cached
	}
	if !a.symbolTypeResolutionIsSafe(symbol) {
		return nil
	}
	a.noteMaterializingRead()
	return a.tp.checker.GetTypeOfSymbol(symbol)
}

// typeFromNodeSafely returns the checker type of an annotation node, preferring
// an already resolved node and otherwise resolving it through the ordinary lazy
// accessor when the recursion guard establishes that the resolution is bounded.
func (a *apiStabilityAnalysis) typeFromNodeSafely(node *ast.Node, subst apiStabilitySubstitution) *checker.Type {
	if node == nil {
		return nil
	}
	if cached := a.tp.checker.GetResolvedTypeFromTypeNode(node); cached != nil {
		return cached
	}
	if !a.annotationResolutionIsSafe(node, subst) {
		return nil
	}
	a.noteMaterializingRead()
	return a.tp.checker.GetTypeFromTypeNode(node)
}

// declaredTypeSafely returns a symbol's declared type, preferring an already
// computed result and otherwise materializing it through the ordinary lazy
// accessor when the recursion guard establishes that the declaration resolution
// is bounded.
func (a *apiStabilityAnalysis) declaredTypeSafely(symbol *ast.Symbol) *checker.Type {
	if symbol == nil {
		return nil
	}
	if cached := a.tp.checker.GetResolvedDeclaredTypeOfSymbolIfMaterialized(symbol); cached != nil {
		return cached
	}
	if !a.declaredTypeResolutionIsSafe(symbol) {
		return nil
	}
	a.noteMaterializingSymbolRead(apiStabilityMaterializationReadDeclaredType, symbol)
	return a.tp.checker.GetDeclaredTypeOfSymbol(symbol)
}

// returnTypeSafely returns a signature's return type, preferring an already
// computed result and otherwise materializing it through the ordinary lazy
// accessor when the pre-flight recursion guard establishes that the resolution
// is bounded. An inferred return is read through the checker's own lazy
// inference; its diagnostics belong to the declaring file exactly as its
// ordinary check would report them.
func (a *apiStabilityAnalysis) returnTypeSafely(signature *checker.Signature, subst apiStabilitySubstitution) *checker.Type {
	if signature == nil {
		return nil
	}
	if cached := a.tp.checker.GetResolvedReturnTypeOfSignatureIfMaterialized(signature); cached != nil {
		return cached
	}
	if !a.signatureReturnResolutionIsSafe(signature, subst) {
		return nil
	}
	a.noteMaterializingRead()
	return a.tp.checker.GetReturnTypeOfSignature(signature)
}

// baseTypesSafely returns a declaration's base types, preferring already
// resolved base types and otherwise materializing the heritage through the
// ordinary lazy accessor when the recursion guard establishes that the
// resolution is bounded.
func (a *apiStabilityAnalysis) baseTypesSafely(declaration *checker.Type, subst apiStabilitySubstitution) ([]*checker.Type, bool) {
	if declaration == nil {
		return nil, false
	}
	if bases, resolved := a.tp.checker.GetResolvedBaseTypesOfTypeIfMaterialized(declaration); resolved {
		return bases, true
	}
	if !a.baseTypesResolutionIsSafe(declaration, subst) {
		return nil, false
	}
	a.noteMaterializingRead()
	return a.tp.checker.GetBaseTypes(declaration), true
}

// declaredTypeOfSymbol returns the declared type of a class, interface, enum
// or type alias, preferring an already computed result and otherwise
// materializing it through the ordinary lazy accessor when the recursion guard
// establishes that the declaration resolution is bounded. Building a declared
// type does not expand its members; member and signature components are only
// read when the public surface actually reaches them.
func (a *apiStabilityAnalysis) declaredTypeOfSymbol(symbol *ast.Symbol) *checker.Type {
	return a.declaredTypeSafely(symbol)
}

// valueTypeOfSymbol returns the value type of a symbol, preferring an already
// computed result and otherwise materializing it through the ordinary lazy
// accessor when the recursion guard establishes that the annotation resolution
// is bounded. It is only called for symbols with a value side, so a type-only
// export is never forced into a value lookup.
func (a *apiStabilityAnalysis) valueTypeOfSymbol(symbol *ast.Symbol) *checker.Type {
	if symbol == nil || symbol.Flags&ast.SymbolFlagsValue == 0 {
		return nil
	}
	return a.typeOfSymbolSafely(symbol)
}

// representedTypeFromNode returns the checker type of an annotation node under
// the analysis safety guard. The node is never used as a syntax-level dependency
// source: the returned represented type is what the surface traversal walks.
func (a *apiStabilityAnalysis) representedTypeFromNode(node *ast.Node, subst apiStabilitySubstitution) *checker.Type {
	return a.typeFromNodeSafely(node, subst)
}

// collectTypeAliasDeclarationSurface reads the public surface of a type alias
// whose declared type is not available from the checker. Only declaration
// `@stability` tags are metadata-read here; the surface stays incomplete
// because the represented type the traversal needs is genuinely unavailable.
func (a *apiStabilityAnalysis) collectTypeAliasDeclarationSurface(surface *apiStabilitySurface, symbol *ast.Symbol, subst apiStabilitySubstitution) bool {
	declaration := apiStabilityTypeAliasDeclaration(symbol)
	if declaration == nil {
		return false
	}
	rhs := declaration.Type()
	if rhs == nil {
		return false
	}
	if represented := a.representedTypeFromNode(rhs, subst); represented != nil {
		surface.merge(a.typeSurface(represented, apiStabilityExpand, subst))
		return true
	}
	a.collectDeclaredTagSurface(surface, rhs)
	return false
}

// collectDeclaredTagSurface reads `@stability` tags from the declaration
// structure of an annotation whose represented type is unavailable. Tags are
// metadata: no annotation is resolved, no referenced declaration is followed,
// and no substitution is built. Conditional branches are visited only when the
// conditional is deferred, so a tag that the compiler eliminated on a concrete
// conditional is never reported.
func (a *apiStabilityAnalysis) collectDeclaredTagSurface(surface *apiStabilitySurface, node *ast.Node) {
	if node == nil {
		return
	}
	switch node.Kind {
	case ast.KindParenthesizedType:
		a.collectDeclaredTagSurface(surface, node.AsParenthesizedTypeNode().Type)
	case ast.KindTypeLiteral:
		for _, member := range node.Members() {
			if member == nil {
				continue
			}
			if member.FunctionLikeData() != nil {
				a.recordDeclarationStability(surface, member)
				continue
			}
			switch member.Kind {
			case ast.KindPropertySignature:
				a.recordDeclarationStability(surface, member)
				a.collectDeclaredTagSurface(surface, member.AsPropertySignatureDeclaration().Type)
			case ast.KindIndexSignature:
				a.recordDeclarationStability(surface, member)
			}
		}
	case ast.KindFunctionType, ast.KindConstructorType:
		a.recordDeclarationStability(surface, node)
	case ast.KindConditionalType:
		if !a.conditionalDeclarationIsDeferred(node) {
			return
		}
		conditional := node.AsConditionalTypeNode()
		a.collectDeclaredTagSurface(surface, conditional.TrueType)
		a.collectDeclaredTagSurface(surface, conditional.FalseType)
	case ast.KindUnionType:
		if list := node.AsUnionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				a.collectDeclaredTagSurface(surface, member)
			}
		}
	case ast.KindIntersectionType:
		if list := node.AsIntersectionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				a.collectDeclaredTagSurface(surface, member)
			}
		}
	case ast.KindArrayType:
		a.collectDeclaredTagSurface(surface, node.AsArrayTypeNode().ElementType)
	case ast.KindTupleType:
		if elements := node.AsTupleTypeNode().Elements; elements != nil {
			for _, element := range elements.Nodes {
				a.collectDeclaredTagSurface(surface, element)
			}
		}
	case ast.KindNamedTupleMember:
		a.collectDeclaredTagSurface(surface, node.AsNamedTupleMember().Type)
	case ast.KindOptionalType:
		a.collectDeclaredTagSurface(surface, node.AsOptionalTypeNode().Type)
	case ast.KindRestType:
		a.collectDeclaredTagSurface(surface, node.AsRestTypeNode().Type)
	case ast.KindTypeOperator:
		a.collectDeclaredTagSurface(surface, node.AsTypeOperatorNode().Type)
	}
}

// typeSurface computes the surface of a represented type in this analysis. A
// complete, context-free result is first looked up in the per-checker shared
// caches and composed from the immutable snapshot plus the component's declared
// tag when one exists; a result computed under a substitution carrier or cut by
// an active type or by the analysis bound stays analysis-local, is returned
// incomplete where relevant, and is settled before it is reused.
func (a *apiStabilityAnalysis) typeSurface(t *checker.Type, inspect apiStabilityInspection, subst apiStabilitySubstitution) apiStabilitySurface {
	if t == nil {
		return newApiStabilitySurface()
	}
	key := apiStabilityTypeKey{t: t, inspect: inspect, carrier: subst.carrier()}
	if cached, ok := a.sessionTypes[key]; ok {
		return cached
	}
	if key.carrier == nil {
		if shared, ok := a.sharedTypeSurface(t, inspect); ok {
			a.storeTypeSurface(key, shared)
			return shared
		}
	}
	a.typeFrames[key] = subst.frame
	if a.activeTypes[key] {
		return cycleCutSurface()
	}
	if !a.consumeWork() {
		return blockedSurface()
	}
	a.activeTypes[key] = true
	startEpoch := a.materializationEpoch
	surface := a.collectTypeSurface(t, inspect, subst)
	delete(a.activeTypes, key)
	if surface.blocked {
		surface.observedEpoch = startEpoch
	}
	a.storeTypeSurface(key, surface)
	return surface
}

func (a *apiStabilityAnalysis) collectTypeSurface(t *checker.Type, inspect apiStabilityInspection, subst apiStabilitySubstitution) apiStabilitySurface {
	surface := newApiStabilitySurface()
	if t == nil || !a.consumeWork() {
		if t != nil {
			surface.block()
		}
		return surface
	}

	// A component reached through the shallow named-reference boundary honors
	// its own explicit declared tag: the tag contributes at its declared level
	// and the component's internals are not expanded. Independently represented
	// type arguments stay exposed. Roots are never reached with the shallow
	// boundary, so a root tag never stops the root walk.
	if inspect == apiStabilityShallow {
		if identity := a.typeChildBoundarySymbol(t); identity != nil {
			a.recordSymbol(&surface, identity)
			a.collectChildTypeArguments(&surface, t, subst)
			return surface
		}
	}

	// An alias keeps its symbol and represented type arguments. The underlying
	// representation is still inspected below; the alias is recorded first so an
	// erased or shallow alias can still be named. A repeated application of the
	// same concrete alias object in the same concrete carrier exposes nothing
	// new, which terminates recursive aliases such as `Deep<T>` without
	// evaluating them.
	if alias := t.Alias(); alias != nil {
		a.recordSymbol(&surface, alias.Symbol())
		if !a.aliasArgumentsAreSelf(alias) && apiStabilityRepresentedSurfaceAcceptsArguments(t) {
			for _, argument := range alias.TypeArguments() {
				surface.merge(a.typeSurface(argument, apiStabilityShallow, subst))
			}
		}
		aliasKey := apiStabilityAliasKey{t: t, carrier: subst.carrier()}
		if a.activeAliases[aliasKey] {
			surface.cutByCycle()
			return surface
		}
		a.activeAliases[aliasKey] = true
		defer delete(a.activeAliases, aliasKey)
	}

	flags := t.Flags()
	switch {
	case flags&checker.TypeFlagsTypeParameter != 0:
		a.recordSymbol(&surface, t.Symbol())
		if mapped, parent, ok := a.substitute(subst, t); ok {
			if mapped == nil {
				// The parameter was replaced by a represented argument the
				// checker has not materialized. The component is unknown, so
				// the result is incomplete and never promoted.
				surface.block()
				return surface
			}
			surface.merge(a.typeSurface(mapped, apiStabilityShallow, parent))
			return surface
		}
		a.collectTypeParameterComponents(&surface, t, subst)
	case flags&checker.TypeFlagsUnionOrIntersection != 0:
		for _, member := range t.Types() {
			surface.merge(a.typeSurface(member, apiStabilityShallow, subst))
		}
	case flags&checker.TypeFlagsConditional != 0:
		a.collectConditionalSurface(&surface, t, subst)
	case flags&checker.TypeFlagsIndexedAccess != 0:
		if indexed := t.AsIndexedAccessType(); indexed != nil {
			surface.merge(a.typeSurface(indexed.ObjectType(), apiStabilityShallow, subst))
			surface.merge(a.typeSurface(indexed.IndexType(), apiStabilityShallow, subst))
		}
		// The projected result of an indexed access is a compiler outcome that
		// the represented operands do not establish; inspecting the operands
		// alone can never complete the surface.
		surface.block()
	case flags&checker.TypeFlagsIndex != 0:
		if index := t.AsIndexType(); index != nil {
			surface.merge(a.typeSurface(index.Target(), apiStabilityShallow, subst))
		}
	case flags&checker.TypeFlagsTemplateLiteral != 0:
		if template := t.AsTemplateLiteralType(); template != nil {
			for _, part := range template.Types() {
				surface.merge(a.typeSurface(part, apiStabilityShallow, subst))
			}
		}
	case flags&checker.TypeFlagsStringMapping != 0:
		a.recordSymbol(&surface, t.Symbol())
		if mapping := t.AsStringMappingType(); mapping != nil {
			surface.merge(a.typeSurface(mapping.Target(), apiStabilityShallow, subst))
		}
	case flags&checker.TypeFlagsSubstitution != 0:
		if substitution := t.AsSubstitutionType(); substitution != nil {
			surface.merge(a.typeSurface(substitution.BaseType(), apiStabilityShallow, subst))
			surface.merge(a.typeSurface(substitution.SubstConstraint(), apiStabilityShallow, subst))
		}
	case flags&checker.TypeFlagsObject != 0:
		a.collectObjectSurface(&surface, t, inspect, subst)
	case flags&(checker.TypeFlagsUniqueESSymbol|checker.TypeFlagsEnum) != 0:
		a.recordSymbol(&surface, t.Symbol())
	}
	return surface
}

// collectTypeParameterComponents records the constraint and default of a type
// parameter, preferring cached checker materializations and otherwise
// materializing the annotation through the ordinary lazy accessor. An
// annotation that would expand a recursive alias, or one that is genuinely
// unavailable, leaves the surface incomplete.
func (a *apiStabilityAnalysis) collectTypeParameterComponents(surface *apiStabilitySurface, t *checker.Type, subst apiStabilitySubstitution) {
	c := a.tp.checker
	constraint := c.GetResolvedConstraintOfTypeParameterIfMaterialized(t)
	if constraint == nil {
		constraint = a.representedTypeFromNode(typeParameterAnnotationNode(t, true), subst)
	}
	if constraint != nil {
		surface.merge(a.typeSurface(constraint, apiStabilityShallow, subst))
	} else if typeParameterAnnotationNode(t, true) != nil {
		surface.block()
	}
	defaultType := c.GetResolvedDefaultFromTypeParameterIfMaterialized(t)
	if defaultType == nil {
		defaultType = a.representedTypeFromNode(typeParameterAnnotationNode(t, false), subst)
	}
	if defaultType != nil {
		surface.merge(a.typeSurface(defaultType, apiStabilityShallow, subst))
	} else if typeParameterAnnotationNode(t, false) != nil {
		surface.block()
	}
}

// collectConditionalSurface inspects a represented conditional type without
// guessing its selected branch. A conditional whose operands are substituted by
// an active concrete application, or whose operands are fully concrete, has an
// outcome the compiler relation selects; unless that outcome is already
// represented on the type, the surface is blocked instead of guessed. A
// deferred symbolic conditional (its operands still mention an unbound type
// parameter) keeps its operands and both declared branches; the declared branch
// components are materialized lazily through the checker's ordinary reads.
func (a *apiStabilityAnalysis) collectConditionalSurface(surface *apiStabilitySurface, t *checker.Type, subst apiStabilitySubstitution) {
	conditional := t.AsConditionalType()
	if conditional == nil {
		return
	}
	// A conditional re-entered on the same recursion path (a recursive alias
	// whose represented branch resolves back to the same type) has already
	// contributed its direct components at the first occurrence. The cut stays
	// incomplete until the settle fixpoint has unified the findings.
	conditionalKey := apiStabilityAliasKey{t: t, carrier: subst.carrier()}
	if a.activeConditionals[conditionalKey] {
		surface.cutByCycle()
		return
	}
	a.activeConditionals[conditionalKey] = true
	defer delete(a.activeConditionals, conditionalKey)

	check := conditional.CheckType()
	extends := conditional.ExtendsType()
	if t.Mapper() != nil ||
		a.containsSubstitutedParameter(subst, check, make(map[*checker.Type]bool)) ||
		a.containsSubstitutedParameter(subst, extends, make(map[*checker.Type]bool)) {
		// A concrete application: the selected branch is a compiler outcome
		// that is not represented on the raw conditional. Never guess it.
		surface.block()
		return
	}
	if !a.representedContainsTypeParameter(check, make(map[*checker.Type]bool)) &&
		!a.representedContainsTypeParameter(extends, make(map[*checker.Type]bool)) {
		// A conditional over fully concrete operands also has an outcome that
		// is not represented; refuse to guess as well.
		surface.block()
		return
	}
	surface.merge(a.typeSurface(check, apiStabilityShallow, subst))
	surface.merge(a.typeSurface(extends, apiStabilityShallow, subst))
	a.collectConditionalBranchSurface(surface, t, true, subst)
	a.collectConditionalBranchSurface(surface, t, false, subst)
}

// containsSubstitutedParameter reports whether any type parameter reachable in
// the represented operand graph is replaced by the active substitution. It only
// reads represented components; it never evaluates a relation.
func (a *apiStabilityAnalysis) containsSubstitutedParameter(subst apiStabilitySubstitution, t *checker.Type, active map[*checker.Type]bool) bool {
	if t == nil || active[t] {
		return false
	}
	active[t] = true
	defer delete(active, t)
	if t.Flags()&checker.TypeFlagsTypeParameter != 0 {
		_, _, ok := a.substitute(subst, t)
		return ok
	}
	switch {
	case t.Flags()&checker.TypeFlagsUnionOrIntersection != 0:
		for _, member := range t.Types() {
			if a.containsSubstitutedParameter(subst, member, active) {
				return true
			}
		}
	case t.Flags()&checker.TypeFlagsObject != 0 && t.ObjectFlags()&checker.ObjectFlagsReference != 0:
		for _, argument := range a.tp.checker.GetResolvedTypeArguments(t) {
			if a.containsSubstitutedParameter(subst, argument, active) {
				return true
			}
		}
	case t.Flags()&checker.TypeFlagsIndexedAccess != 0:
		if indexed := t.AsIndexedAccessType(); indexed != nil {
			return a.containsSubstitutedParameter(subst, indexed.ObjectType(), active) ||
				a.containsSubstitutedParameter(subst, indexed.IndexType(), active)
		}
	case t.Flags()&checker.TypeFlagsIndex != 0:
		if index := t.AsIndexType(); index != nil {
			return a.containsSubstitutedParameter(subst, index.Target(), active)
		}
	case t.Flags()&checker.TypeFlagsTemplateLiteral != 0:
		if template := t.AsTemplateLiteralType(); template != nil {
			for _, part := range template.Types() {
				if a.containsSubstitutedParameter(subst, part, active) {
					return true
				}
			}
		}
	case t.Flags()&checker.TypeFlagsConditional != 0:
		if conditional := t.AsConditionalType(); conditional != nil {
			return a.containsSubstitutedParameter(subst, conditional.CheckType(), active) ||
				a.containsSubstitutedParameter(subst, conditional.ExtendsType(), active)
		}
	}
	return false
}

// representedContainsTypeParameter reports whether an unbound type parameter is
// a reachable component of the represented operand graph.
func (a *apiStabilityAnalysis) representedContainsTypeParameter(t *checker.Type, active map[*checker.Type]bool) bool {
	if t == nil || active[t] {
		return false
	}
	active[t] = true
	defer delete(active, t)
	if t.Flags()&checker.TypeFlagsTypeParameter != 0 {
		return true
	}
	switch {
	case t.Flags()&checker.TypeFlagsUnionOrIntersection != 0:
		for _, member := range t.Types() {
			if a.representedContainsTypeParameter(member, active) {
				return true
			}
		}
	case t.Flags()&checker.TypeFlagsObject != 0 && t.ObjectFlags()&checker.ObjectFlagsReference != 0:
		for _, argument := range a.tp.checker.GetResolvedTypeArguments(t) {
			if a.representedContainsTypeParameter(argument, active) {
				return true
			}
		}
	case t.Flags()&checker.TypeFlagsIndexedAccess != 0:
		if indexed := t.AsIndexedAccessType(); indexed != nil {
			return a.representedContainsTypeParameter(indexed.ObjectType(), active) ||
				a.representedContainsTypeParameter(indexed.IndexType(), active)
		}
	case t.Flags()&checker.TypeFlagsIndex != 0:
		if index := t.AsIndexType(); index != nil {
			return a.representedContainsTypeParameter(index.Target(), active)
		}
	case t.Flags()&checker.TypeFlagsTemplateLiteral != 0:
		if template := t.AsTemplateLiteralType(); template != nil {
			for _, part := range template.Types() {
				if a.representedContainsTypeParameter(part, active) {
					return true
				}
			}
		}
	case t.Flags()&checker.TypeFlagsConditional != 0:
		if conditional := t.AsConditionalType(); conditional != nil {
			return a.representedContainsTypeParameter(conditional.CheckType(), active) ||
				a.representedContainsTypeParameter(conditional.ExtendsType(), active)
		}
	case t.Flags()&checker.TypeFlagsSubstitution != 0:
		if substitution := t.AsSubstitutionType(); substitution != nil {
			return a.representedContainsTypeParameter(substitution.BaseType(), active) ||
				a.representedContainsTypeParameter(substitution.SubstConstraint(), active)
		}
	}
	return false
}

// collectConditionalBranchSurface inspects one declared branch of a deferred
// conditional. The checker's already resolved branch components are preferred;
// otherwise the declared branch annotation is materialized lazily through the
// ordinary type accessor and traversed as a represented component. A primitive
// keyword branch exposes nothing. A genuinely unavailable branch leaves the
// surface incomplete.
func (a *apiStabilityAnalysis) collectConditionalBranchSurface(surface *apiStabilitySurface, t *checker.Type, trueBranch bool, subst apiStabilitySubstitution) {
	c := a.tp.checker
	node := c.GetConditionalTypeBranchNode(t, trueBranch)
	if node == nil {
		surface.block()
		return
	}
	represented := c.GetResolvedConditionalTypeBranch(t, trueBranch)
	if represented == nil {
		represented = a.representedTypeFromNode(node, subst)
	}
	if represented == nil {
		if apiStabilityPrimitiveTypeNode(node) {
			return
		}
		surface.block()
		return
	}
	surface.merge(a.typeSurface(represented, apiStabilityShallow, subst))
	a.collectTypeNodeProvenance(surface, node, represented, subst)
}

// apiStabilityPrimitiveTypeNode reports whether a type node resolves to a
// primitive singleton that carries no symbol or member surface. Those nodes do
// not cache a resolved type and expose nothing.
func apiStabilityPrimitiveTypeNode(node *ast.Node) bool {
	if node == nil {
		return false
	}
	switch node.Kind {
	case ast.KindAnyKeyword, ast.KindUnknownKeyword, ast.KindNeverKeyword, ast.KindVoidKeyword,
		ast.KindUndefinedKeyword, ast.KindNullKeyword, ast.KindStringKeyword, ast.KindNumberKeyword,
		ast.KindBigIntKeyword, ast.KindBooleanKeyword, ast.KindSymbolKeyword, ast.KindObjectKeyword:
		return true
	}
	return false
}

// collectObjectSurface handles object types with the represented structure
// rules used by the data-first matcher. A class or interface declaration is
// inspected through its declared members and heritage clauses; an expanded
// reference prefers its concrete materialized surface and otherwise
// materializes it lazily. A named reference reached in a shallow position
// keeps its declaration, represented arguments and callable signatures and its
// members are not expanded. Recursive alias expansions are refused by the
// lazy guards.
func (a *apiStabilityAnalysis) collectObjectSurface(surface *apiStabilitySurface, t *checker.Type, inspect apiStabilityInspection, subst apiStabilitySubstitution) {
	flags := t.ObjectFlags()
	if flags&(checker.ObjectFlagsReverseMapped|checker.ObjectFlagsEvolvingArray) != 0 {
		return
	}
	if flags&checker.ObjectFlagsInstantiationExpressionType != 0 {
		// An instantiation expression such as `identity<E>` represents an
		// explicitly instantiated callable value whose signatures are safe to
		// read from the checker's represented members.
		a.collectFunctionSignatures(surface, t, t, subst)
		return
	}
	// A class declared instance type is a class-or-interface surface inspected
	// through its declaration. The class static side is a separate anonymous
	// object type whose symbol is the class.
	if flags&checker.ObjectFlagsClassOrInterface != 0 {
		a.collectDeclarationSurface(surface, t, inspect, subst)
		return
	}
	if flags&checker.ObjectFlagsAnonymous != 0 {
		if symbol := t.Symbol(); symbol != nil && symbol.Flags&ast.SymbolFlagsClass != 0 {
			a.collectClassStaticSurface(surface, t, inspect, subst)
			return
		}
	}
	// A module namespace is a named reference to a whole module's exports, not a
	// finite anonymous surface. Recording the module symbol keeps the traversal
	// bounded instead of enumerating and reporting every exported member; the
	// rule enumerates namespace members explicitly.
	if symbol := t.Symbol(); symbol != nil &&
		symbol.Flags&(ast.SymbolFlagsModule|ast.SymbolFlagsValueModule|ast.SymbolFlagsNamespaceModule) != 0 {
		a.recordSymbol(surface, symbol)
		return
	}
	if flags&checker.ObjectFlagsReference != 0 && t.Target() != nil && t.Target() != t {
		target := t.Target()
		if a.shouldInspectSymbol(target.Symbol()) {
			a.recordSymbol(surface, target.Symbol())
		}
		arguments := a.referenceArguments(t, target)
		for _, argument := range arguments {
			if argument == nil {
				continue
			}
			surface.merge(a.typeSurface(argument, apiStabilityShallow, subst))
		}
		if target.ObjectFlags()&checker.ObjectFlagsClassOrInterface != 0 {
			if !a.shouldInspectSymbol(target.Symbol()) {
				// A default-library target contributes only its represented
				// type arguments; its member surface is never expanded.
				return
			}
			if inspect == apiStabilityShallow {
				// The shallow boundary: declared symbol, represented type
				// arguments and directly exposed call and construct signatures
				// only. The raw declaration surface is read with the reference
				// substitution, so an instantiated signature argument or return
				// stays represented without expanding arbitrary members.
				referenceSubst := a.extendParametersSubstitution(subst, t, a.referenceTypeParameters(target), arguments)
				a.collectDeclarationSurface(surface, target, apiStabilityShallow, referenceSubst)
				return
			}
			a.collectConcreteReferenceSurface(surface, t)
			return
		}
		return
	}
	if flags&checker.ObjectFlagsMapped != 0 {
		a.collectMappedSurface(surface, t, subst)
		return
	}
	surface.merge(a.componentSurface(t, inspect, subst))
}

// collectConcreteReferenceSurface inspects the concrete public surface of a
// class or interface reference in an expanded context. The checker's resolved
// member table is preferred and read directly; the pre-flight recursion guard
// is only consulted when an individual accessor has not been materialized yet.
// Each unresolved member, signature and index read shares one completed Safe
// verdict for the same reference and substitution, so an already resolved
// accessor never consumes safety work. An unmaterialized member table whose
// member resolution is not established as bounded falls back to the target
// declaration under the reference substitution with its own gated reads, and a
// surface whose member or index resolution would evaluate a recursive alias is
// left incomplete. Concrete members are already instantiated, so no
// substitution applies to them; nested named references reached inside those
// members still use the shallow boundary.
func (a *apiStabilityAnalysis) collectConcreteReferenceSurface(surface *apiStabilitySurface, reference *checker.Type) {
	c := a.tp.checker
	empty := apiStabilitySubstitution{}
	var target *checker.Type
	var referenceSubst apiStabilitySubstitution
	if reference.ObjectFlags()&checker.ObjectFlagsReference != 0 {
		target = reference.Target()
	}
	owner := reference.Symbol()
	if target != nil && target != reference && target.ObjectFlags()&checker.ObjectFlagsClassOrInterface != 0 {
		referenceSubst = a.extendParametersSubstitution(empty, reference, a.referenceTypeParameters(target), a.referenceArguments(reference, target))
	}
	if members, resolved := c.GetResolvedMembersOfTypeIfMaterialized(reference); resolved {
		a.collectResolvedMemberTable(surface, members, owner, empty)
	} else {
		structuredSafe := a.memberTableResolutionIsSafe(reference, referenceSubst)
		switch {
		case structuredSafe:
			a.noteMaterializingRead()
			for _, member := range c.GetPropertiesOfType(reference) {
				if member == nil || member.Flags&ast.SymbolFlagsTypeParameter != 0 || apiStabilitySymbolIsNonPublic(member) {
					continue
				}
				if boundary := a.inheritedChildBoundarySymbol(member, owner); boundary != nil {
					a.recordSymbol(surface, boundary)
					continue
				}
				surface.merge(a.memberSurface(member, empty))
			}
		case target != nil && target != reference && target.ObjectFlags()&checker.ObjectFlagsClassOrInterface != 0:
			// The declaration surface under the reference substitution never
			// resolves the instantiated member table; each member and index read
			// is gated on its own.
			a.collectDeclarationSurface(surface, target, apiStabilityExpand, referenceSubst)
			return
		default:
			surface.block()
		}
	}
	// The resolved member table has already flattened inherited members; the
	// tagged ancestors still expose their independently represented type
	// arguments even though their members stay bounded.
	a.collectTaggedHeritageArguments(surface, target, referenceSubst, make(map[*checker.Type]bool))
	for _, kind := range []checker.SignatureKind{checker.SignatureKindCall, checker.SignatureKindConstruct} {
		if signatures, ok := c.GetResolvedSignaturesOfTypeIfMaterialized(reference, kind); ok {
			for _, signature := range signatures {
				if a.childSignatureBoundary(surface, signature, owner) {
					continue
				}
				surface.merge(a.signatureSurface(signature, empty))
			}
			continue
		}
		if !a.memberTableResolutionIsSafe(reference, referenceSubst) {
			surface.block()
			continue
		}
		a.noteMaterializingRead()
		for _, signature := range c.GetSignaturesOfType(reference, kind) {
			if a.childSignatureBoundary(surface, signature, owner) {
				continue
			}
			surface.merge(a.signatureSurface(signature, empty))
		}
	}
	if infos, ok := c.GetResolvedIndexInfosOfTypeIfMaterialized(reference); ok {
		for _, info := range infos {
			if info == nil {
				continue
			}
			a.collectIndexInfoSurface(surface, info, empty, owner)
		}
		return
	}
	if !a.memberTableResolutionIsSafe(reference, referenceSubst) {
		surface.block()
		return
	}
	a.noteMaterializingRead()
	for _, info := range c.GetIndexInfosOfType(reference) {
		if info == nil {
			continue
		}
		a.collectIndexInfoSurface(surface, info, empty, owner)
	}
}

// collectResolvedMemberTable merges the public members of an already resolved
// member table with the given substitution. Compiler-reserved internal entries
// are skipped, while a late-bound computed member is traversed like any other
// member: it is the resolved spelling of a public computed declaration and its
// visibility is decided by its declaration provenance. A member the compiler
// flattened into this table from an explicitly tagged base is a child boundary:
// the base's declared level contributes and the member's surface is not
// expanded. A same-named own override keeps the owner as its declaring symbol
// and stays inspected.
func (a *apiStabilityAnalysis) collectResolvedMemberTable(surface *apiStabilitySurface, members ast.SymbolTable, owner *ast.Symbol, subst apiStabilitySubstitution) {
	for _, name := range sortedSymbolTableKeys(members) {
		member := members[name]
		if member == nil || member.Flags&ast.SymbolFlagsTypeParameter != 0 || apiStabilitySymbolIsNonPublic(member) {
			continue
		}
		if !apiStabilityMemberTableNameIsVisible(name, member) {
			continue
		}
		if boundary := a.inheritedChildBoundarySymbol(member, owner); boundary != nil {
			a.recordSymbol(surface, boundary)
			continue
		}
		surface.merge(a.memberSurface(member, subst))
	}
}

// referenceArguments returns the represented type arguments of a reference,
// filling omitted arguments from the target's already materialized type
// parameter defaults. A parameter the checker left unrepresented stays nil so
// the substitution machinery blocks instead of assuming a constraint or
// default.
func (a *apiStabilityAnalysis) referenceArguments(reference, target *checker.Type) []*checker.Type {
	if target == nil || target.Flags()&checker.TypeFlagsObject == 0 || target.ObjectFlags()&checker.ObjectFlagsClassOrInterface == 0 {
		return a.tp.checker.GetResolvedTypeArguments(reference)
	}
	parameters := a.referenceTypeParameters(target)
	if len(parameters) == 0 {
		return nil
	}
	resolved := a.tp.checker.GetResolvedTypeArguments(reference)
	arguments := make([]*checker.Type, len(parameters))
	for index, parameter := range parameters {
		if index < len(resolved) && resolved[index] != nil {
			arguments[index] = resolved[index]
			continue
		}
		if defaultType := a.tp.checker.GetResolvedDefaultFromTypeParameterIfMaterialized(parameter); defaultType != nil {
			arguments[index] = defaultType
		}
	}
	return arguments
}

// referenceTypeParameters returns the type parameters of a class or interface
// reference target. Other reference targets (tuples, arrays of primitives) have
// no bindable type parameters and return nil.
func (a *apiStabilityAnalysis) referenceTypeParameters(t *checker.Type) []*checker.Type {
	if t == nil || t.Flags()&checker.TypeFlagsObject == 0 || t.ObjectFlags()&checker.ObjectFlagsClassOrInterface == 0 {
		return nil
	}
	return t.AsInterfaceType().TypeParameters()
}

// collectDeclarationSurface inspects the public surface declared by a class or
// interface declaration: its own members, index signatures and signatures,
// its type parameters, and (when expanded) its inherited members. The
// declaration type passed here is always uninstantiated; the substitution
// context carries the represented type arguments of the reference that reached
// it.
func (a *apiStabilityAnalysis) collectDeclarationSurface(surface *apiStabilitySurface, declaration *checker.Type, inspect apiStabilityInspection, subst apiStabilitySubstitution) {
	if declaration == nil {
		return
	}
	c := a.tp.checker
	symbol := declaration.Symbol()
	if symbol == nil || !a.shouldInspectSymbol(symbol) {
		return
	}
	a.recordSymbol(surface, symbol)
	key := apiStabilitySymbolKey{symbol: symbol, carrier: subst.carrier()}
	if a.activeSymbols[key] {
		surface.cutByCycle()
		return
	}
	a.activeSymbols[key] = true
	defer delete(a.activeSymbols, key)

	if inspect == apiStabilityShallow {
		// A named dependency keeps the shallow boundary: its own public call and
		// construct signatures (and their inherited signatures) are part of the
		// represented surface, but its arbitrary members are not expanded.
		a.collectDeclarationSignatures(surface, symbol, subst, false)
		a.collectHeritage(surface, declaration, apiStabilityShallow, subst)
		return
	}

	if iface := declaration.AsInterfaceType(); iface != nil {
		for _, typeParameter := range iface.LocalTypeParameters() {
			surface.merge(a.typeSurface(typeParameter, apiStabilityShallow, subst))
		}
	}

	switch {
	case a.collectResolvedMemberTableIfMaterialized(surface, declaration, symbol, subst):
	case symbol.Members != nil:
		a.collectResolvedMemberTable(surface, symbol.Members, symbol, subst)
		// A computed member name that the checker must evaluate to resolve the
		// member table is not part of the early table; the surface is incomplete
		// and is never treated as stable.
		if a.symbolHasLateBoundMembers(symbol) {
			surface.block()
		}
	case a.memberTableResolutionIsSafe(declaration, subst):
		members := a.materializedMembersOfSymbol(c, symbol)
		if members != nil {
			a.collectResolvedMemberTable(surface, members, symbol, subst)
		} else {
			surface.block()
		}
	default:
		surface.block()
	}

	a.collectDeclarationSignatures(surface, symbol, subst, false)
	a.collectIndexInfosOfDeclaration(surface, declaration, symbol, subst)
	a.collectHeritage(surface, declaration, inspect, subst)
}

// memberSurface inspects one class or interface member. The member's own
// declared `@stability` tag is metadata and always contributes; when the tag is
// explicit the member is a child boundary for its own represented type and its
// internals are not expanded. Otherwise the member's represented type is
// preferred from the checker's cache and otherwise materialized through the
// ordinary lazy accessor; a member that genuinely has no type still exposes its
// raw declaration signature.
func (a *apiStabilityAnalysis) memberSurface(member *ast.Symbol, subst apiStabilitySubstitution) apiStabilitySurface {
	surface := newApiStabilitySurface()
	if member == nil {
		return surface
	}
	a.recordSymbol(&surface, member)
	if declared := a.tp.DeclaredApiStabilityOfSymbol(member); declared.Declaration != nil {
		return surface
	}
	memberType := a.typeOfSymbolSafely(member)
	if memberType == nil {
		// A method or accessor whose function type is genuinely unavailable
		// still exposes its raw declaration signature. The signature belongs to
		// the class or interface that declares the member, so that declaring
		// symbol is the owner comparison: a tagged declaring class never bounds
		// its own member's raw signature.
		signatureOwner := a.memberDeclaringSymbol(member)
		if signatureOwner == nil {
			signatureOwner = member
		}
		if signatures := a.materializedSignaturesOfSymbol(a.tp.checker, member); len(signatures) != 0 {
			for _, signature := range signatures {
				if a.childSignatureBoundary(&surface, signature, signatureOwner) {
					continue
				}
				surface.merge(a.signatureSurface(signature, subst))
			}
			return surface
		}
		surface.block()
		return surface
	}
	surface.merge(a.typeSurface(memberType, apiStabilityShallow, subst))
	for _, declaration := range member.Declarations {
		if !a.shouldInspectMemberDeclaration(declaration) {
			continue
		}
		a.collectDeclarationProvenance(&surface, declaration, memberType, subst)
	}
	return surface
}

// collectDeclarationSignatures collects a declaration's own public call and
// construct signatures. Construct signatures declared under the class
// constructor member are only included when includeClassConstructors is set;
// a class instance surface does not expose them, while the static side does.
// The early member table is read directly: call, construct and constructor
// symbols are never late-bound, so inspecting them must not resolve a
// late-bound computed-name member table.
func (a *apiStabilityAnalysis) collectDeclarationSignatures(surface *apiStabilitySurface, symbol *ast.Symbol, subst apiStabilitySubstitution, includeClassConstructors bool) {
	if symbol == nil {
		return
	}
	c := a.tp.checker
	members := symbol.Members
	if members == nil {
		return
	}
	internals := []string{ast.InternalSymbolNameCall, ast.InternalSymbolNameNew}
	if includeClassConstructors {
		internals = append(internals, ast.InternalSymbolNameConstructor)
	}
	for _, internal := range internals {
		member := members[internal]
		if member == nil {
			continue
		}
		if !a.signatureMemberResolutionIsSafe(member, subst) {
			surface.block()
			continue
		}
		for _, signature := range a.materializedSignaturesOfSymbol(c, member) {
			if a.childSignatureBoundary(surface, signature, symbol) {
				continue
			}
			surface.merge(a.signatureSurface(signature, subst))
		}
	}
}

// collectIndexInfosOfDeclaration reads a declaration's index signatures. The
// already resolved index infos are used when the checker materialized them;
// otherwise the declared index signatures contribute their own `@stability`
// tag and their materialized key/value annotations, and an unmaterialized
// annotation leaves the surface incomplete. The resolved infos include index
// signatures the compiler flattened in from inherited bases; each info is
// attributed to its declaring class or interface so an explicitly tagged base's
// index signature stays a child boundary. The declared-only fallback carries
// own declarations, so warm and cold runs agree on that boundary.
func (a *apiStabilityAnalysis) collectIndexInfosOfDeclaration(surface *apiStabilitySurface, declaration *checker.Type, symbol *ast.Symbol, subst apiStabilitySubstitution) {
	c := a.tp.checker
	if infos, ok := c.GetResolvedIndexInfosOfTypeIfMaterialized(declaration); ok {
		for _, info := range infos {
			if info == nil {
				continue
			}
			a.collectIndexInfoSurface(surface, info, subst, symbol)
		}
		return
	}
	infos := c.GetDeclaredIndexInfosOfSymbol(symbol)
	for _, info := range infos {
		if info == nil || info.Declaration() == nil {
			continue
		}
		a.collectIndexSignatureSurface(surface, info.Declaration(), info.KeyType(), info.ValueType(), subst, symbol)
	}
}

// collectIndexInfoSurface inspects one already resolved index info. The index
// declaration's own `@stability` tag always contributes; when the tag is
// explicit the index signature is a child boundary and its key and value
// internals are not expanded. A resolved index info the compiler flattened in
// from an explicitly tagged base (its declaring class or interface differs from
// owner) is bounded the same way. A key or value type the checker has not
// resolved is materialized lazily from the declaration when possible.
func (a *apiStabilityAnalysis) collectIndexInfoSurface(surface *apiStabilitySurface, info *checker.IndexInfo, subst apiStabilitySubstitution, owner *ast.Symbol) {
	if info == nil {
		return
	}
	declaration := info.Declaration()
	if a.declarationChildBoundary(surface, declaration, owner) {
		return
	}
	keyType := info.KeyType()
	if keyType == nil {
		keyType = a.representedTypeFromNode(apiStabilityIndexSignatureKeyNode(declaration), subst)
	}
	valueType := info.ValueType()
	if valueType == nil {
		valueType = a.representedTypeFromNode(apiStabilityIndexSignatureValueNode(declaration), subst)
	}
	if keyType != nil {
		surface.merge(a.typeSurface(keyType, apiStabilityShallow, subst))
	} else {
		surface.block()
	}
	if valueType != nil {
		surface.merge(a.typeSurface(valueType, apiStabilityShallow, subst))
		a.collectTypeNodeProvenance(surface, apiStabilityIndexSignatureValueNode(declaration), valueType, subst)
	} else {
		surface.block()
	}
}

// collectIndexSignatureSurface inspects one declared index signature. The key
// and value types are used when the checker has already represented them;
// otherwise the declaration's annotations are materialized lazily. An explicit
// `@stability` tag makes the signature a child boundary: the tag contributes
// and its key and value internals are not expanded, and a declaration whose
// declaring class or interface is explicitly tagged (and differs from owner) is
// bounded the same way.
func (a *apiStabilityAnalysis) collectIndexSignatureSurface(surface *apiStabilitySurface, declaration *ast.Node, keyType, valueType *checker.Type, subst apiStabilitySubstitution, owner *ast.Symbol) {
	if declaration == nil {
		return
	}
	if a.declarationChildBoundary(surface, declaration, owner) {
		return
	}
	indexSignature := declaration.AsIndexSignatureDeclaration()
	if indexSignature == nil {
		return
	}
	if indexSignature.Parameters != nil {
		for _, parameter := range indexSignature.Parameters.Nodes {
			if parameter == nil || parameter.Kind != ast.KindParameter {
				continue
			}
			typeNode := parameter.AsParameterDeclaration().Type
			if keyType == nil {
				keyType = a.representedTypeFromNode(typeNode, subst)
			}
			if keyType != nil {
				surface.merge(a.typeSurface(keyType, apiStabilityShallow, subst))
				a.collectTypeNodeProvenance(surface, typeNode, keyType, subst)
			} else {
				surface.block()
			}
		}
	}
	if valueType == nil {
		valueType = a.representedTypeFromNode(indexSignature.Type, subst)
	}
	if valueType != nil {
		surface.merge(a.typeSurface(valueType, apiStabilityShallow, subst))
		a.collectTypeNodeProvenance(surface, indexSignature.Type, valueType, subst)
		return
	}
	surface.block()
}

// collectHeritage inspects a declaration's inherited surface. Base types the
// checker has already resolved are preferred; otherwise the declaration's
// heritage is materialized through the ordinary lazy accessor. The clauses are
// never resolved from syntax.
func (a *apiStabilityAnalysis) collectHeritage(surface *apiStabilitySurface, declaration *checker.Type, inspect apiStabilityInspection, subst apiStabilitySubstitution) {
	bases, resolved := a.tp.checker.GetResolvedBaseTypesOfTypeIfMaterialized(declaration)
	if !resolved {
		if !apiStabilitySymbolDeclaresHeritage(declaration.Symbol()) {
			return
		}
		lazyBases, ok := a.baseTypesSafely(declaration, subst)
		if !ok {
			surface.block()
			return
		}
		bases = lazyBases
	}
	for _, base := range bases {
		if base == nil {
			continue
		}
		target := base
		if base.ObjectFlags()&checker.ObjectFlagsReference != 0 && base.Target() != nil {
			target = base.Target()
		}
		if target == nil || target.ObjectFlags()&checker.ObjectFlagsClassOrInterface == 0 {
			surface.merge(a.typeSurface(base, apiStabilityShallow, subst))
			continue
		}
		if boundary := a.heritageChildBoundarySymbol(base, target); boundary != nil {
			// An explicitly tagged base is a child boundary: its declared level
			// contributes and its internals are not expanded, while its
			// independently represented type arguments stay exposed.
			a.recordSymbol(surface, boundary)
			a.collectChildTypeArguments(surface, base, subst)
			continue
		}
		baseSubst := a.extendParametersSubstitution(subst, base, a.referenceTypeParameters(target), a.referenceArguments(base, target))
		a.collectDeclarationSurface(surface, target, inspect, baseSubst)
	}
}

// collectClassStaticSurface inspects the class static side. Its members are
// declared on the class symbol's exports and cannot depend on class type
// parameters. The instance side of a class value is included when the class
// value is directly exported, because `typeof C` exposes it through
// construction.
func (a *apiStabilityAnalysis) collectClassStaticSurface(surface *apiStabilitySurface, t *checker.Type, inspect apiStabilityInspection, subst apiStabilitySubstitution) {
	symbol := t.Symbol()
	if symbol == nil || !a.shouldInspectSymbol(symbol) {
		return
	}
	c := a.tp.checker
	a.recordSymbol(surface, symbol)
	if inspect == apiStabilityExpand {
		if instance := a.declaredTypeSafely(symbol); instance != nil {
			surface.merge(a.typeSurface(instance, apiStabilityExpand, subst))
		}
		// The class static members of a directly expanded class value are part
		// of its own surface. A nested `typeof C` keeps the shallow boundary:
		// its declared symbol and directly exposed call and construct
		// signatures are inspected, but its static members are not expanded.
		if members, ok := c.GetResolvedMembersOfTypeIfMaterialized(t); ok {
			for _, name := range sortedSymbolTableKeys(members) {
				if name == "prototype" {
					continue
				}
				member := members[name]
				if member == nil || apiStabilitySymbolIsNonPublic(member) {
					continue
				}
				if !apiStabilityMemberTableNameIsVisible(name, member) {
					continue
				}
				if boundary := a.inheritedChildBoundarySymbol(member, symbol); boundary != nil {
					a.recordSymbol(surface, boundary)
					continue
				}
				surface.merge(a.memberSurface(member, apiStabilitySubstitution{}))
			}
		} else if symbol.Exports != nil {
			for _, name := range sortedSymbolTableKeys(symbol.Exports) {
				if name == "prototype" {
					continue
				}
				member := symbol.Exports[name]
				if member == nil || apiStabilitySymbolIsNonPublic(member) {
					continue
				}
				if !apiStabilityMemberTableNameIsVisible(name, member) {
					continue
				}
				if boundary := a.inheritedChildBoundarySymbol(member, symbol); boundary != nil {
					a.recordSymbol(surface, boundary)
					continue
				}
				surface.merge(a.memberSurface(member, apiStabilitySubstitution{}))
			}
			// The early exports table omits computed members (unique-symbol and
			// dynamic names); the surface is incomplete until the checker
			// late-binds them.
			if a.symbolHasLateBoundMembers(symbol) {
				surface.block()
			}
		}
	}
	a.collectDeclarationSignatures(surface, symbol, subst, true)
	for _, kind := range []checker.SignatureKind{checker.SignatureKindCall, checker.SignatureKindConstruct} {
		if signatures, ok := c.GetResolvedSignaturesOfTypeIfMaterialized(t, kind); ok {
			for _, signature := range signatures {
				if a.childSignatureBoundary(surface, signature, symbol) {
					continue
				}
				surface.merge(a.signatureSurface(signature, subst))
			}
		}
	}
}

// collectAnonymousSurface inspects a finite anonymous object type. The concrete
// member surface of an instantiated anonymous type is preferred and read
// directly when materialized. An unmaterialized instantiated member surface is
// materialized through the ordinary lazy property accessor only when the
// recursion guard establishes that member resolution is bounded; otherwise the
// raw target members are read under the mapper substitution with their own
// gated reads. A raw anonymous type (the target of an instantiation reached
// through a heritage or mapper path) is inspected through its declared members
// under the active substitution.
func (a *apiStabilityAnalysis) collectAnonymousSurface(surface *apiStabilitySurface, t *checker.Type, subst apiStabilitySubstitution) {
	c := a.tp.checker
	instantiated := t.ObjectFlags()&checker.ObjectFlagsInstantiated != 0 && t.Target() != nil
	source := t
	current := subst
	if instantiated {
		source = t.Target()
		if mapper := t.Mapper(); mapper != nil {
			current = a.extendMapperSubstitution(subst, t, mapper)
		}
	}
	a.recordSymbol(surface, source.Symbol())
	if instantiated {
		empty := apiStabilitySubstitution{}
		switch {
		case a.collectResolvedMemberTableIfMaterialized(surface, t, source.Symbol(), empty):
		case a.memberTableResolutionIsSafe(t, current):
			a.noteMaterializingRead()
			for _, member := range c.GetPropertiesOfType(t) {
				if member == nil || member.Flags&ast.SymbolFlagsTypeParameter != 0 || apiStabilitySymbolIsNonPublic(member) {
					continue
				}
				if boundary := a.inheritedChildBoundarySymbol(member, source.Symbol()); boundary != nil {
					a.recordSymbol(surface, boundary)
					continue
				}
				surface.merge(a.memberSurface(member, empty))
			}
		case source.Symbol() != nil && source.Symbol().Members != nil:
			a.collectResolvedMemberTable(surface, source.Symbol().Members, source.Symbol(), current)
			if a.symbolHasLateBoundMembers(source.Symbol()) {
				surface.block()
			}
		default:
			surface.block()
		}
		a.collectAnonymousIndexInfos(surface, t, source, empty, current)
		a.collectFunctionSignatures(surface, t, source, current)
		return
	}
	if source.Symbol() != nil {
		switch {
		case a.collectResolvedMemberTableIfMaterialized(surface, source, source.Symbol(), current):
		case source.Symbol().Members != nil:
			a.collectResolvedMemberTable(surface, source.Symbol().Members, source.Symbol(), current)
			if a.symbolHasLateBoundMembers(source.Symbol()) {
				surface.block()
			}
		case a.memberTableResolutionIsSafe(source, current):
			members := a.materializedMembersOfSymbol(c, source.Symbol())
			if members != nil {
				a.collectResolvedMemberTable(surface, members, source.Symbol(), current)
			}
			// A function-like symbol has no member table of its own; its call
			// signatures are declared directly on the symbol and are read by
			// collectFunctionSignatures.
		default:
			surface.block()
		}
	}
	a.collectAnonymousIndexInfos(surface, t, source, current, current)
	a.collectFunctionSignatures(surface, t, source, current)
}

// collectResolvedMemberTableIfMaterialized merges the checker's already
// resolved member table when one exists.
func (a *apiStabilityAnalysis) collectResolvedMemberTableIfMaterialized(surface *apiStabilitySurface, t *checker.Type, owner *ast.Symbol, subst apiStabilitySubstitution) bool {
	if members, ok := a.tp.checker.GetResolvedMembersOfTypeIfMaterialized(t); ok {
		a.collectResolvedMemberTable(surface, members, owner, subst)
		return true
	}
	return false
}

// collectAnonymousIndexInfos reads the index signatures of an anonymous type.
// The resolved infos of the (possibly instantiated) type are preferred; when
// the checker has not resolved them, the raw declared infos contribute under
// the mapper substitution and their annotations are read through the guard.
func (a *apiStabilityAnalysis) collectAnonymousIndexInfos(surface *apiStabilitySurface, t, source *checker.Type, subst, mapperSubst apiStabilitySubstitution) {
	c := a.tp.checker
	var owner *ast.Symbol
	if source != nil {
		owner = source.Symbol()
	}
	if infos, ok := c.GetResolvedIndexInfosOfTypeIfMaterialized(t); ok {
		resolvedSubst := subst
		if source != t {
			resolvedSubst = apiStabilitySubstitution{}
		}
		for _, info := range infos {
			if info == nil {
				continue
			}
			a.collectIndexInfoSurface(surface, info, resolvedSubst, owner)
		}
		return
	}
	if source == nil || source.Symbol() == nil {
		return
	}
	if source != t {
		a.collectIndexInfosOfSymbol(surface, source.Symbol(), mapperSubst)
		return
	}
	a.collectIndexInfosOfSymbol(surface, source.Symbol(), subst)
}

// collectIndexInfosOfSymbol contributes the declared index signatures of a
// symbol under a substitution context. Key and value annotations are read
// through the recursion guard; an unavailable annotation leaves the surface
// incomplete.
func (a *apiStabilityAnalysis) collectIndexInfosOfSymbol(surface *apiStabilitySurface, symbol *ast.Symbol, subst apiStabilitySubstitution) {
	if symbol == nil {
		return
	}
	for _, info := range a.tp.checker.GetDeclaredIndexInfosOfSymbol(symbol) {
		if info == nil || info.Declaration() == nil {
			continue
		}
		a.collectIndexSignatureSurface(surface, info.Declaration(), info.KeyType(), info.ValueType(), subst, symbol)
	}
}

// collectFunctionSignatures reads the call and construct signatures of an
// anonymous function-like type. Resolved signatures are preferred. An
// unmaterialized surface is read from the raw declaration signatures under the
// active substitution: creating a raw signature resolves its parameter
// annotations but never instantiates the enclosing application, and the
// guarded per-signature reads stay bounded.
func (a *apiStabilityAnalysis) collectFunctionSignatures(surface *apiStabilitySurface, t, source *checker.Type, subst apiStabilitySubstitution) {
	c := a.tp.checker
	owner := (*ast.Symbol)(nil)
	if source != nil {
		owner = source.Symbol()
	}
	for _, kind := range []checker.SignatureKind{checker.SignatureKindCall, checker.SignatureKindConstruct} {
		if signatures, ok := c.GetResolvedSignaturesOfTypeIfMaterialized(t, kind); ok {
			for _, signature := range signatures {
				if a.childSignatureBoundary(surface, signature, owner) {
					continue
				}
				surface.merge(a.signatureSurface(signature, subst))
			}
			continue
		}
		if source == nil || source.Symbol() == nil {
			surface.block()
			continue
		}
		internal := ast.InternalSymbolNameCall
		if kind == checker.SignatureKindConstruct {
			internal = ast.InternalSymbolNameNew
		}
		symbol := source.Symbol()
		member := symbol.Members[internal]
		if member == nil && kind == checker.SignatureKindCall &&
			symbol.Flags&(ast.SymbolFlagsFunction|ast.SymbolFlagsMethod|ast.SymbolFlagsGetAccessor|ast.SymbolFlagsSetAccessor|ast.SymbolFlagsAccessor) != 0 {
			// A function or accessor declaration keeps its call signatures on
			// its own symbol rather than in a member table.
			member = symbol
		}
		if member == nil {
			continue
		}
		if !a.signatureMemberResolutionIsSafe(member, subst) {
			surface.block()
			continue
		}
		for _, signature := range a.materializedSignaturesOfSymbol(c, member) {
			if a.childSignatureBoundary(surface, signature, owner) {
				continue
			}
			surface.merge(a.signatureSurface(signature, subst))
		}
	}
}

// collectMappedSurface reads the represented mapped type components. An
// instantiated mapped type is read through its target so the mapped parameter
// stays uninstantiated and the mapper provides the substitution. Cached
// components are preferred; a missing component is materialized through the
// ordinary mapped accessors unless its annotation would expand a recursive
// alias. Whatever remains unavailable leaves the surface incomplete.
func (a *apiStabilityAnalysis) collectMappedSurface(surface *apiStabilitySurface, t *checker.Type, subst apiStabilitySubstitution) {
	a.recordSymbol(surface, t.Symbol())
	source := t
	current := subst
	if t.ObjectFlags()&checker.ObjectFlagsInstantiated != 0 && t.Target() != nil {
		source = t.Target()
		if mapper := t.Mapper(); mapper != nil {
			current = a.extendMapperSubstitution(subst, t, mapper)
		}
	}
	mapped := source.AsMappedType()
	if mapped == nil {
		return
	}
	var constraintNode, nameNode, templateNode *ast.Node
	if symbol := source.Symbol(); symbol != nil {
		for _, declaration := range symbol.Declarations {
			if declaration == nil || declaration.Kind != ast.KindMappedType {
				continue
			}
			mappedDeclaration := declaration.AsMappedTypeNode()
			if mappedDeclaration == nil {
				continue
			}
			if mappedDeclaration.TypeParameter != nil {
				if typeParameter := mappedDeclaration.TypeParameter.AsTypeParameterDeclaration(); typeParameter != nil {
					constraintNode = typeParameter.Constraint
				}
			}
			nameNode = mappedDeclaration.NameType
			templateNode = mappedDeclaration.Type
			break
		}
	}
	constraint := mapped.ConstraintType()
	if constraint == nil {
		constraint = a.typeFromNodeSafely(constraintNode, current)
	}
	if constraint != nil {
		surface.merge(a.typeSurface(constraint, apiStabilityShallow, current))
	} else {
		surface.block()
	}
	name := mapped.NameType()
	if name == nil {
		name = a.typeFromNodeSafely(nameNode, current)
	}
	if name != nil {
		surface.merge(a.typeSurface(name, apiStabilityShallow, current))
	} else if nameNode != nil || apiStabilityMappedDeclarationHasName(source) {
		surface.block()
	}
	template := mapped.TemplateType()
	if template == nil {
		template = a.typeFromNodeSafely(templateNode, current)
	}
	if template != nil {
		surface.merge(a.typeSurface(template, apiStabilityShallow, current))
	} else {
		surface.block()
	}
}

// componentSurface inspects a finite anonymous component exactly once per
// recursion path and substitution context.
func (a *apiStabilityAnalysis) componentSurface(t *checker.Type, inspect apiStabilityInspection, subst apiStabilitySubstitution) apiStabilitySurface {
	if t == nil {
		return newApiStabilitySurface()
	}
	key := apiStabilityTypeKey{t: t, inspect: inspect, carrier: subst.carrier()}
	if cached, ok := a.sessionComponents[key]; ok {
		return cached
	}
	a.typeFrames[key] = subst.frame
	if a.activeComponents[key] {
		return cycleCutSurface()
	}
	if !a.consumeWork() {
		return blockedSurface()
	}
	a.activeComponents[key] = true
	startEpoch := a.materializationEpoch
	surface := a.collectObjectComponentSurface(t, inspect, subst)
	delete(a.activeComponents, key)
	if surface.blocked {
		surface.observedEpoch = startEpoch
	}
	a.storeComponentSurface(key, surface)
	return surface
}

// collectObjectComponentSurface is used when a represented object type must be
// inspected as a finite anonymous surface (mapped targets, reference targets
// that are anonymous). Class and interface declarations never reach this path.
func (a *apiStabilityAnalysis) collectObjectComponentSurface(t *checker.Type, inspect apiStabilityInspection, subst apiStabilitySubstitution) apiStabilitySurface {
	surface := newApiStabilitySurface()
	if t == nil {
		return surface
	}
	if t.ObjectFlags()&checker.ObjectFlagsClassOrInterface != 0 {
		// A class or interface target is inspected through its declaration, not
		// through the resolved member surface.
		a.collectDeclarationSurface(&surface, t, inspect, subst)
		return surface
	}
	if symbol := t.Symbol(); symbol != nil && symbol.Flags&ast.SymbolFlagsClass != 0 {
		a.collectClassStaticSurface(&surface, t, inspect, subst)
		return surface
	}
	a.collectAnonymousSurface(&surface, t, subst)
	return surface
}

// signatureSurface reads a signature's uninstantiated target together with its
// represented substitution. Using the target keeps a generic signature's
// parameter and return types uninstantiated (so a recursive alias is never
// evaluated), while the substitution frame retains the instantiated context
// (so an inferred return such as `(x: T) => T` instantiated with `E` still
// exposes `E`). The concrete signature pointer and the substitution context are
// both part of the cache key, so the same raw signature analyzed under
// different enclosing mappers never shares a cached result. A complete,
// context-free concrete signature surface is looked up in the per-checker
// shared caches and composed from the immutable snapshot plus the signature's
// declared tag; a result computed under a substitution carrier stays
// analysis-local.
func (a *apiStabilityAnalysis) signatureSurface(signature *checker.Signature, subst apiStabilitySubstitution) apiStabilitySurface {
	if signature == nil {
		return newApiStabilitySurface()
	}
	raw := rawSignature(signature)
	if raw == nil || apiStabilitySignatureIsNonPublic(raw) {
		return newApiStabilitySurface()
	}
	key := apiStabilitySignatureKey{signature: signature, carrier: subst.carrier()}
	if cached, ok := a.sessionSignatures[key]; ok {
		return cached
	}
	if key.carrier == nil {
		if shared, ok := a.sharedSignatureSurface(signature); ok {
			a.storeSignatureSurface(key, shared)
			return shared
		}
	}
	a.signatureFrames[key] = subst.frame
	if a.activeSignatures[key] {
		return cycleCutSurface()
	}
	if !a.consumeWork() {
		return blockedSurface()
	}
	a.activeSignatures[key] = true
	startEpoch := a.materializationEpoch
	surface := a.collectSignatureSurface(raw, signature, subst)
	delete(a.activeSignatures, key)
	if surface.blocked {
		surface.observedEpoch = startEpoch
	}
	a.storeSignatureSurface(key, surface)
	return surface
}

func (a *apiStabilityAnalysis) collectSignatureSurface(raw *checker.Signature, concrete *checker.Signature, subst apiStabilitySubstitution) apiStabilitySurface {
	surface := newApiStabilitySurface()
	if raw == nil {
		return surface
	}
	a.recordSignatureStability(&surface, raw)

	signatureSubst := subst
	var concreteParameters []*ast.Symbol
	var concreteThis *ast.Symbol
	if concrete != nil && concrete != raw {
		signatureSubst = a.extendSignatureSubstitution(subst, concrete)
		// The concrete signature's type arguments are represented in the
		// enclosing context; a compound argument is traversed shallowly.
		for _, argument := range a.signatureArguments(concrete) {
			surface.merge(a.typeSurface(argument, apiStabilityShallow, subst))
		}
		concreteParameters = concrete.Parameters()
		concreteThis = concrete.ThisParameter()
	}
	for _, typeParameter := range raw.TypeParameters() {
		if _, _, replaced := a.substitute(signatureSubst, typeParameter); replaced {
			// A bound argument replaces the declaration; its constraint and default
			// are not part of the instantiated surface.
			continue
		}
		surface.merge(a.typeSurface(typeParameter, apiStabilityShallow, signatureSubst))
	}
	collectParameter := func(rawParameter, concreteParameter *ast.Symbol) {
		// Prefer the concrete signature's represented parameter component: the
		// instantiated parameter carries the compiler's actual outcome. When
		// the read is cut by a recursive application the raw parameter's
		// represented component is used with the signature substitution, so a
		// bare type parameter still maps to its concrete argument without
		// instantiating a hidden recursive alias.
		if concreteParameter != nil {
			if concreteType := a.typeOfSymbolSafely(concreteParameter); concreteType != nil {
				surface.merge(a.typeSurface(concreteType, apiStabilityShallow, subst))
				return
			}
		}
		if parameterType, known := a.parameterRepresentedType(rawParameter); known {
			if parameterType != nil {
				surface.merge(a.typeSurface(parameterType, apiStabilityShallow, signatureSubst))
			}
		} else {
			surface.block()
		}
	}
	if thisParameter := raw.ThisParameter(); thisParameter != nil {
		collectParameter(thisParameter, concreteThis)
	}
	rawParameters := raw.Parameters()
	for index, parameter := range rawParameters {
		var concreteParameter *ast.Symbol
		if index < len(concreteParameters) {
			concreteParameter = concreteParameters[index]
		}
		collectParameter(parameter, concreteParameter)
	}
	if concrete != nil && concrete != raw {
		// Prefer the concrete signature's represented return: the
		// instantiated return carries the compiler's actual outcome. When the
		// read is cut by a recursive application the raw declaration's
		// uninstantiated return annotation is used with the signature
		// substitution instead.
		represented := a.returnTypeSafely(concrete, signatureSubst)
		if represented != nil {
			surface.merge(a.typeSurface(represented, apiStabilityShallow, subst))
		} else {
			a.collectReturnTypeSurface(&surface, raw, signatureSubst)
		}
	} else {
		a.collectReturnTypeSurface(&surface, raw, signatureSubst)
	}
	a.collectSignatureProvenance(&surface, raw, signatureSubst)
	return surface
}

// collectReturnTypeSurface inspects the public return type of a signature. The
// signature's materialized return type is preferred; otherwise it is
// materialized lazily, with the already resolved annotation node, a primitive
// keyword, or a bare type parameter binder as fallbacks. A genuinely
// unavailable return leaves the surface incomplete.
func (a *apiStabilityAnalysis) collectReturnTypeSurface(surface *apiStabilitySurface, signature *checker.Signature, subst apiStabilitySubstitution) {
	if signature == nil {
		return
	}
	represented, known := a.returnRepresentedType(signature, subst)
	if !known {
		surface.block()
		return
	}
	if represented != nil {
		surface.merge(a.typeSurface(represented, apiStabilityShallow, subst))
	}
}

// returnRepresentedType returns the return type component of a signature. The
// boolean is false only when the component is genuinely unavailable, which
// blocks the surface; a primitive keyword is known and exposes nothing.
func (a *apiStabilityAnalysis) returnRepresentedType(signature *checker.Signature, subst apiStabilitySubstitution) (*checker.Type, bool) {
	if signature == nil {
		return nil, false
	}
	if resolved := a.returnTypeSafely(signature, subst); resolved != nil {
		return resolved, true
	}
	node := signatureReturnTypeNode(signature)
	if node == nil {
		return nil, false
	}
	if represented := a.tp.checker.GetResolvedTypeFromTypeNode(node); represented != nil {
		return represented, true
	}
	if apiStabilityPrimitiveTypeNode(node) {
		return nil, true
	}
	if binder := a.typeParameterFromAnnotation(node); binder != nil {
		return binder, true
	}
	return nil, false
}

// parameterTypeOf returns the represented type of a parameter, or nil when the
// parameter exposes no type component of its own (a primitive keyword) or is
// unavailable.
func (a *apiStabilityAnalysis) parameterTypeOf(parameter *ast.Symbol) *checker.Type {
	represented, _ := a.parameterRepresentedType(parameter)
	return represented
}

// parameterRepresentedType returns the represented type component of a
// parameter. The parameter's materialized type is preferred and otherwise
// materialized lazily; the already resolved annotation node type and a bare
// type parameter binder are fallbacks, while a primitive keyword is known and
// exposes nothing.
func (a *apiStabilityAnalysis) parameterRepresentedType(parameter *ast.Symbol) (*checker.Type, bool) {
	if parameter == nil {
		return nil, false
	}
	if materialized := a.tp.checker.GetResolvedTypeOfSymbolIfMaterialized(parameter); materialized != nil {
		return materialized, true
	}
	if resolved := a.typeOfSymbolSafely(parameter); resolved != nil {
		return resolved, true
	}
	for _, declaration := range parameter.Declarations {
		if declaration == nil || declaration.Kind != ast.KindParameter {
			continue
		}
		annotation := declaration.AsParameterDeclaration().Type
		if annotation == nil {
			continue
		}
		if represented := a.tp.checker.GetResolvedTypeFromTypeNode(annotation); represented != nil {
			return represented, true
		}
		if apiStabilityPrimitiveTypeNode(annotation) {
			return nil, true
		}
		if binder := a.typeParameterFromAnnotation(annotation); binder != nil {
			return binder, true
		}
	}
	return nil, false
}

// typeParameterFromAnnotation returns the declared type parameter a type
// annotation names, when the annotation is exactly a bare type parameter
// reference. It reads the annotation symbol only; no annotation type is
// resolved.
func (a *apiStabilityAnalysis) typeParameterFromAnnotation(node *ast.Node) *checker.Type {
	if node == nil || node.Kind != ast.KindTypeReference {
		return nil
	}
	reference := node.AsTypeReferenceNode()
	if reference.TypeArguments != nil && len(reference.TypeArguments.Nodes) != 0 {
		return nil
	}
	symbol := a.symbolAtTypeNameNode(reference.TypeName)
	if symbol == nil || symbol.Flags&ast.SymbolFlagsTypeParameter == 0 {
		return nil
	}
	a.noteMaterializingSymbolRead(apiStabilityMaterializationReadDeclaredType, symbol)
	return a.tp.checker.GetDeclaredTypeOfSymbol(symbol)
}

// signatureArguments returns the checker-supplied type arguments of an
// instantiated signature, or nil when the signature has no represented
// substitution. The result is memoized for the analysis.
func (a *apiStabilityAnalysis) signatureArguments(signature *checker.Signature) []*checker.Type {
	if signature == nil {
		return nil
	}
	if cached, ok := a.signatureArgs[signature]; ok {
		return cached
	}
	arguments := a.tp.checker.GetTypeArgumentsForResolvedSignature(signature)
	a.signatureArgs[signature] = arguments
	return arguments
}

// substitute resolves a type parameter through the active substitution chain.
// A compiler mapper maps the parameter directly; a concrete signature maps its
// own parameters through the concrete type arguments or its instantiated
// parameter mappers; a reference maps its target's parameters through the
// represented arguments. A parameter replaced by an unrepresented argument
// returns a nil mapping with ok set, which blocks the surface.
func (a *apiStabilityAnalysis) substitute(subst apiStabilitySubstitution, t *checker.Type) (*checker.Type, apiStabilitySubstitution, bool) {
	if t == nil || t.Flags()&checker.TypeFlagsTypeParameter == 0 {
		return nil, subst, false
	}
	// A distributive conditional check is the distributed clone of its declared
	// type parameter; its constraint points back at the declaration binder. All
	// frames map the declared binder, so match through it.
	if normalized := checker.GetNonDistributedTypeParameter(a.tp.checker, t); normalized != nil && normalized != t {
		t = normalized
	}
	for frame := subst.frame; frame != nil; frame = frame.parent {
		parent := apiStabilitySubstitution{frame: frame.parent}
		if frame.mapper != nil {
			if mapped := frame.mapper.Map(t); mapped != nil && mapped != t {
				return mapped, parent, true
			}
		}
		if frame.signature != nil {
			if mapped, ok := a.signatureTypeParameter(frame.signature, t); ok && mapped != t {
				return mapped, parent, true
			}
		}
		if frame.parameters != nil {
			for index, parameter := range frame.parameters {
				if parameter != t || index >= len(frame.arguments) {
					continue
				}
				mapped := frame.arguments[index]
				if mapped == nil {
					// The argument was not represented. The parameter is replaced,
					// but its value is unknown, so the component is unknown.
					return nil, parent, true
				}
				if mapped == t {
					continue
				}
				return mapped, parent, true
			}
		}
	}
	return nil, subst, false
}

func (a *apiStabilityAnalysis) signatureTypeParameter(signature *checker.Signature, t *checker.Type) (*checker.Type, bool) {
	raw := rawSignature(signature)
	if raw == nil {
		return nil, false
	}
	if arguments := a.signatureArguments(signature); len(arguments) != 0 {
		parameters := raw.TypeParameters()
		for index, parameter := range parameters {
			if parameter == t && index < len(arguments) {
				return arguments[index], true
			}
		}
	}
	if raw == signature {
		return nil, false
	}
	return a.signatureBoundTypeParameter(signature, t)
}

// signatureBoundTypeParameter resolves a bare type parameter through an
// instantiated signature's own parameter and return types. It only reads a
// concrete type when the corresponding target is itself a bare type parameter,
// and it maps the binder forward with the instantiated symbol's own mapper
// when the concrete type has not been materialized, so a recursive compound
// member is never evaluated. The derived map is memoized per analysis.
func (a *apiStabilityAnalysis) signatureBoundTypeParameter(signature *checker.Signature, t *checker.Type) (*checker.Type, bool) {
	if signature == nil || t == nil || t.Flags()&checker.TypeFlagsTypeParameter == 0 {
		return nil, false
	}
	raw := rawSignature(signature)
	if raw == nil || raw == signature {
		return nil, false
	}
	bound, ok := a.signatureParameterCache[signature]
	if !ok {
		bound = a.computeSignatureParameterMap(raw, signature)
		a.signatureParameterCache[signature] = bound
	}
	mapped, ok := bound[t]
	return mapped, ok
}

func (a *apiStabilityAnalysis) computeSignatureParameterMap(raw *checker.Signature, concrete *checker.Signature) map[*checker.Type]*checker.Type {
	bound := make(map[*checker.Type]*checker.Type)
	c := a.tp.checker
	bind := func(rawParameter, concreteParameter *ast.Symbol) {
		if rawParameter == nil || concreteParameter == nil {
			return
		}
		rawType := a.parameterTypeOf(rawParameter)
		if rawType == nil || rawType.Flags()&checker.TypeFlagsTypeParameter == 0 {
			return
		}
		concreteType := c.GetResolvedTypeOfSymbolIfMaterialized(concreteParameter)
		if concreteType == nil {
			// Prefer the instantiated symbol's own mapper: it maps the binder
			// forward without instantiating a compound annotation.
			if mapper := c.GetInstantiatedSymbolMapper(concreteParameter); mapper != nil {
				concreteType = mapper.Map(rawType)
			}
		}
		if concreteType != nil && concreteType != rawType {
			bound[rawType] = concreteType
		}
	}
	rawParameters, concreteParameters := raw.Parameters(), concrete.Parameters()
	if len(rawParameters) == len(concreteParameters) {
		for index := range rawParameters {
			bind(rawParameters[index], concreteParameters[index])
		}
	}
	bind(raw.ThisParameter(), concrete.ThisParameter())
	rawReturn := a.parameterReturnType(raw, apiStabilitySubstitution{})
	if rawReturn != nil && rawReturn.Flags()&checker.TypeFlagsTypeParameter != 0 {
		concreteReturn := c.GetResolvedReturnTypeOfSignatureIfMaterialized(concrete)
		if concreteReturn != nil && concreteReturn != rawReturn {
			bound[rawReturn] = concreteReturn
		}
	}
	return bound
}

// parameterReturnType returns the represented return type of a signature: the
// signature's materialized return type, the already resolved annotation node
// type, or the declared binder when the return annotation is exactly a type
// parameter reference.
func (a *apiStabilityAnalysis) parameterReturnType(signature *checker.Signature, subst apiStabilitySubstitution) *checker.Type {
	represented, _ := a.returnRepresentedType(signature, subst)
	return represented
}

// collectDeclarationProvenance records the alias symbols written in a public
// declaration's own type annotations. The checker erases an alias such as
// `type ExperimentalText = string`, so symbol provenance is the only way to
// name it. The traversal is driven by the represented checker type and never
// descends into conditional branches, so a symbol that survives only in a
// discarded branch is not reported.
func (a *apiStabilityAnalysis) collectDeclarationProvenance(surface *apiStabilitySurface, declaration *ast.Node, represented *checker.Type, subst apiStabilitySubstitution) {
	if declaration == nil || represented == nil {
		return
	}
	if functionLike := declaration.FunctionLikeData(); functionLike != nil {
		switch declaration.Kind {
		case ast.KindGetAccessor:
			a.collectTypeNodeProvenance(surface, functionLike.Type, represented, subst)
		case ast.KindSetAccessor:
			if functionLike.Parameters != nil {
				for _, parameter := range functionLike.Parameters.Nodes {
					if parameter == nil || parameter.Kind != ast.KindParameter {
						continue
					}
					a.collectTypeNodeProvenance(surface, parameter.AsParameterDeclaration().Type, represented, subst)
				}
			}
		}
		return
	}
	switch declaration.Kind {
	case ast.KindVariableDeclaration:
		a.collectTypeNodeProvenance(surface, declaration.AsVariableDeclaration().Type, represented, subst)
	case ast.KindPropertyDeclaration:
		a.collectTypeNodeProvenance(surface, declaration.AsPropertyDeclaration().Type, represented, subst)
	case ast.KindPropertySignature:
		a.collectTypeNodeProvenance(surface, declaration.AsPropertySignatureDeclaration().Type, represented, subst)
	case ast.KindTypeAliasDeclaration, ast.KindJSTypeAliasDeclaration:
		a.collectTypeNodeProvenance(surface, declaration.Type(), represented, subst)
	case ast.KindIndexSignature:
		a.collectTypeNodeProvenance(surface, declaration.AsIndexSignatureDeclaration().Type, represented, subst)
	}
}

// collectSignatureProvenance records erased alias symbols in a signature's
// public annotations. A type parameter replaced by a represented argument is
// skipped together with its constraint and default, matching the represented
// instantiated surface.
func (a *apiStabilityAnalysis) collectSignatureProvenance(surface *apiStabilitySurface, raw *checker.Signature, signatureSubst apiStabilitySubstitution) {
	declaration := raw.Declaration()
	if declaration == nil {
		return
	}
	functionLike := declaration.FunctionLikeData()
	if functionLike == nil {
		return
	}
	c := a.tp.checker
	if functionLike.TypeParameters != nil {
		rawTypeParameters := raw.TypeParameters()
		for index, typeParameter := range functionLike.TypeParameters.Nodes {
			if typeParameter == nil || typeParameter.Kind != ast.KindTypeParameter || index >= len(rawTypeParameters) {
				continue
			}
			if _, _, replaced := a.substitute(signatureSubst, rawTypeParameters[index]); replaced {
				continue
			}
			typeParameterDeclaration := typeParameter.AsTypeParameterDeclaration()
			if typeParameterDeclaration == nil {
				continue
			}
			a.collectTypeNodeProvenance(
				surface,
				typeParameterDeclaration.Constraint,
				c.GetResolvedConstraintOfTypeParameterIfMaterialized(rawTypeParameters[index]),
				signatureSubst,
			)
			a.collectTypeNodeProvenance(
				surface,
				typeParameterDeclaration.DefaultType,
				c.GetResolvedDefaultFromTypeParameterIfMaterialized(rawTypeParameters[index]),
				signatureSubst,
			)
		}
	}
	rawParameters := raw.Parameters()
	if functionLike.Parameters != nil {
		parameterIndex := 0
		for _, parameter := range functionLike.Parameters.Nodes {
			if parameter == nil || parameter.Kind != ast.KindParameter {
				continue
			}
			parameterDeclaration := parameter.AsParameterDeclaration()
			if parameterDeclaration == nil {
				continue
			}
			represented := (*checker.Type)(nil)
			if parameterIndex < len(rawParameters) {
				represented = a.parameterTypeOf(rawParameters[parameterIndex])
			}
			a.collectTypeNodeProvenance(surface, parameterDeclaration.Type, represented, signatureSubst)
			parameterIndex++
		}
	}
	a.collectTypeNodeProvenance(surface, functionLike.Type, a.parameterReturnType(raw, signatureSubst), signatureSubst)
}

// collectTypeNodeProvenance records alias symbols named by type annotation
// nodes whose represented checker type is directly reachable. It is
// deliberately structural and correlated with the represented types: unions,
// intersections, tuples, arrays, reference arguments and literal members are
// only descended when the corresponding represented component exists, and
// conditional branches are never descended into. A union or intersection
// constituent is correlated by concrete component identity, so a retained
// constituent whose alias the compiler collapsed or reordered is still named.
// An annotation whose represented type is unavailable records nothing, which
// keeps the walk from becoming a syntax dependency parser.
func (a *apiStabilityAnalysis) collectTypeNodeProvenance(surface *apiStabilitySurface, node *ast.Node, represented *checker.Type, subst apiStabilitySubstitution) {
	if node == nil || represented == nil {
		return
	}
	switch node.Kind {
	case ast.KindParenthesizedType:
		a.collectTypeNodeProvenance(surface, node.AsParenthesizedTypeNode().Type, represented, subst)
	case ast.KindTypeReference:
		reference := node.AsTypeReferenceNode()
		a.recordSymbol(surface, a.symbolAtTypeNameNode(reference.TypeName))
		if apiStabilityRepresentedSurfaceAcceptsArguments(represented) {
			a.collectTypeArgumentProvenance(surface, reference.TypeArguments, represented, subst)
		}
	case ast.KindExpressionWithTypeArguments:
		expression := node.AsExpressionWithTypeArguments()
		a.recordSymbol(surface, a.symbolAtTypeNameNode(expression.Expression))
		if apiStabilityRepresentedSurfaceAcceptsArguments(represented) {
			a.collectTypeArgumentProvenance(surface, expression.TypeArguments, represented, subst)
		}
	case ast.KindTypeQuery:
		a.recordSymbol(surface, a.symbolAtTypeNameNode(node.AsTypeQueryNode().ExprName))
	case ast.KindImportType:
		importType := node.AsImportTypeNode()
		if importType.Qualifier != nil {
			a.recordSymbol(surface, a.symbolAtTypeNameNode(importType.Qualifier))
		}
		if apiStabilityRepresentedSurfaceAcceptsArguments(represented) {
			a.collectTypeArgumentProvenance(surface, importType.TypeArguments, represented, subst)
		}
	case ast.KindUnionType:
		if list := node.AsUnionTypeNode().Types; list != nil {
			a.collectAggregateTypeNodeProvenance(surface, list, represented, subst)
		}
	case ast.KindIntersectionType:
		if list := node.AsIntersectionTypeNode().Types; list != nil {
			a.collectAggregateTypeNodeProvenance(surface, list, represented, subst)
		}
	case ast.KindArrayType:
		element := node.AsArrayTypeNode().ElementType
		a.collectTypeNodeProvenance(surface, element, a.representedElementType(represented), subst)
	case ast.KindTupleType:
		elements := node.AsTupleTypeNode().Elements
		if elements == nil {
			return
		}
		representedElements := a.representedTupleArguments(represented)
		for index, element := range elements.Nodes {
			elementType := (*checker.Type)(nil)
			if index < len(representedElements) {
				elementType = representedElements[index]
			}
			a.collectTypeNodeProvenance(surface, element, elementType, subst)
		}
	case ast.KindTypeOperator:
		a.collectTypeNodeProvenance(surface, node.AsTypeOperatorNode().Type, represented, subst)
	case ast.KindIndexedAccessType:
		indexed := node.AsIndexedAccessTypeNode()
		objectType, indexType := apiStabilityRepresentedIndexedAccess(represented)
		a.collectTypeNodeProvenance(surface, indexed.ObjectType, objectType, subst)
		a.collectTypeNodeProvenance(surface, indexed.IndexType, indexType, subst)
	case ast.KindConditionalType:
		conditional := node.AsConditionalTypeNode()
		checkType, extendsType := apiStabilityRepresentedConditionalParts(represented)
		a.collectTypeNodeProvenance(surface, conditional.CheckType, checkType, subst)
		a.collectTypeNodeProvenance(surface, conditional.ExtendsType, extendsType, subst)
	case ast.KindFunctionType, ast.KindConstructorType:
		a.collectFunctionTypeProvenance(surface, node, represented, subst)
	case ast.KindTypeLiteral:
		a.collectTypeLiteralProvenance(surface, node, represented, subst)
	case ast.KindMappedType:
		mapped := node.AsMappedTypeNode()
		var constraint, nameType, templateType *checker.Type
		if represented != nil && represented.ObjectFlags()&checker.ObjectFlagsMapped != 0 {
			mappedType := represented.AsMappedType()
			constraint = mappedType.ConstraintType()
			nameType = mappedType.NameType()
			templateType = mappedType.TemplateType()
		}
		if mapped.TypeParameter != nil {
			if typeParameterDeclaration := mapped.TypeParameter.AsTypeParameterDeclaration(); typeParameterDeclaration != nil {
				// The mapped parameter's constraint is the mapped constraint,
				// already paired above; this pairing only names erased aliases.
				a.collectTypeNodeProvenance(surface, typeParameterDeclaration.Constraint, constraint, subst)
			}
		}
		a.collectTypeNodeProvenance(surface, mapped.NameType, nameType, subst)
		a.collectTypeNodeProvenance(surface, mapped.Type, templateType, subst)
	}
}

func apiStabilityRepresentedMembers(t *checker.Type) []*checker.Type {
	if t == nil {
		return nil
	}
	if t.Flags()&checker.TypeFlagsUnionOrIntersection != 0 {
		return t.Types()
	}
	// Compiler normalization collapsed the aggregate to a single constituent;
	// that constituent is the only retained component.
	return []*checker.Type{t}
}

// apiStabilityRepresentedContainsType reports whether the normalized aggregate
// retained a component with the exact compiler identity of t.
func apiStabilityRepresentedContainsType(members []*checker.Type, t *checker.Type) bool {
	if t == nil {
		return false
	}
	return slices.Contains(members, t)
}

// apiStabilityRepresentedContainsAggregate reports whether every constituent of
// a written union or intersection was retained in the normalized aggregate.
// Compiler normalization flattens nested aggregates, so a written constituent
// such as `A = string | number` is retained as its individual members rather
// than as one aggregate component.
func apiStabilityRepresentedContainsAggregate(members []*checker.Type, t *checker.Type) bool {
	constituents := apiStabilityRepresentedMembers(t)
	if len(constituents) == 0 {
		return false
	}
	for _, constituent := range constituents {
		if !apiStabilityRepresentedContainsType(members, constituent) {
			return false
		}
	}
	return true
}

// collectAggregateTypeNodeProvenance correlates the written constituents of a
// union or intersection annotation with the represented aggregate by concrete
// component identity. Compiler normalization can remove, collapse, reorder or
// combine constituents, so pairing by position would attach the wrong
// represented component. A constituent whose resolved type is retained in the
// normalized aggregate is descended so its erased alias stays named; a
// constituent eliminated by normalization records nothing. The node type is
// read through the ordinary lazy accessor, correlated with a retained
// component, and only then walked; the node itself never introduces a
// dependency the represented type does not contain.
func (a *apiStabilityAnalysis) collectAggregateTypeNodeProvenance(surface *apiStabilitySurface, list *ast.NodeList, represented *checker.Type, subst apiStabilitySubstitution) {
	members := apiStabilityRepresentedMembers(represented)
	for _, member := range list.Nodes {
		if member == nil {
			continue
		}
		memberType := a.representedTypeFromNode(member, subst)
		if memberType == nil ||
			(!apiStabilityRepresentedContainsType(members, memberType) && !apiStabilityRepresentedContainsAggregate(members, memberType)) {
			continue
		}
		a.collectTypeNodeProvenance(surface, member, memberType, subst)
	}
}

func apiStabilityRepresentedIndexedAccess(t *checker.Type) (*checker.Type, *checker.Type) {
	if t != nil && t.Flags()&checker.TypeFlagsIndexedAccess != 0 {
		if indexed := t.AsIndexedAccessType(); indexed != nil {
			return indexed.ObjectType(), indexed.IndexType()
		}
	}
	return nil, nil
}

func apiStabilityRepresentedConditionalParts(t *checker.Type) (*checker.Type, *checker.Type) {
	if t != nil && t.Flags()&checker.TypeFlagsConditional != 0 {
		if conditional := t.AsConditionalType(); conditional != nil {
			return conditional.CheckType(), conditional.ExtendsType()
		}
	}
	return nil, nil
}

// apiStabilityRepresentedSurfaceAcceptsArguments reports whether the written
// type arguments of a named reference are part of the represented surface. An
// application the checker evaluated away (an eliminated conditional resolving
// to `never` or to a primitive) exposes none of its arguments, so recording
// them would report a symbol the surface cannot expose.
func apiStabilityRepresentedSurfaceAcceptsArguments(represented *checker.Type) bool {
	if represented == nil {
		return true
	}
	flags := represented.Flags()
	return flags&(checker.TypeFlagsNever|checker.TypeFlagsString|checker.TypeFlagsNumber|
		checker.TypeFlagsBoolean|checker.TypeFlagsBigInt|checker.TypeFlagsESSymbol|
		checker.TypeFlagsVoid|checker.TypeFlagsUndefined|checker.TypeFlagsNull) == 0
}

func (a *apiStabilityAnalysis) collectTypeArgumentProvenance(surface *apiStabilitySurface, arguments *ast.NodeList, represented *checker.Type, subst apiStabilitySubstitution) {
	if arguments == nil || len(arguments.Nodes) == 0 {
		return
	}
	representedArguments := a.representedTypeArguments(represented)
	for index, argument := range arguments.Nodes {
		argumentType := (*checker.Type)(nil)
		if index < len(representedArguments) {
			argumentType = representedArguments[index]
		}
		a.collectTypeNodeProvenance(surface, argument, argumentType, subst)
	}
}

// collectFunctionTypeProvenance pairs a function-like annotation's parameter
// and return nodes with the represented signature when one is available. When
// it is not, the annotation records nothing: nodes are never resolved here.
func (a *apiStabilityAnalysis) collectFunctionTypeProvenance(surface *apiStabilitySurface, node *ast.Node, represented *checker.Type, subst apiStabilitySubstitution) {
	c := a.tp.checker
	kind := checker.SignatureKindCall
	if node.Kind == ast.KindConstructorType {
		kind = checker.SignatureKindConstruct
	}
	var raw *checker.Signature
	if represented != nil {
		if signatures, ok := c.GetResolvedSignaturesOfTypeIfMaterialized(represented, kind); ok && len(signatures) != 0 {
			raw = rawSignature(signatures[0])
		}
	}
	if raw == nil {
		return
	}
	if functionLike := node.FunctionLikeData(); functionLike != nil && functionLike.TypeParameters != nil {
		rawTypeParameters := raw.TypeParameters()
		for index, typeParameter := range functionLike.TypeParameters.Nodes {
			if typeParameter == nil || typeParameter.Kind != ast.KindTypeParameter || index >= len(rawTypeParameters) {
				continue
			}
			typeParameterDeclaration := typeParameter.AsTypeParameterDeclaration()
			if typeParameterDeclaration == nil {
				continue
			}
			a.collectTypeNodeProvenance(surface, typeParameterDeclaration.Constraint, c.GetResolvedConstraintOfTypeParameterIfMaterialized(rawTypeParameters[index]), subst)
			a.collectTypeNodeProvenance(surface, typeParameterDeclaration.DefaultType, c.GetResolvedDefaultFromTypeParameterIfMaterialized(rawTypeParameters[index]), subst)
		}
	}
	rawParameters := raw.Parameters()
	parameters := node.Parameters()
	for index, parameter := range parameters {
		if parameter == nil || parameter.Kind != ast.KindParameter {
			continue
		}
		parameterType := (*checker.Type)(nil)
		if index < len(rawParameters) {
			parameterType = a.parameterTypeOf(rawParameters[index])
		}
		a.collectTypeNodeProvenance(surface, parameter.AsParameterDeclaration().Type, parameterType, subst)
	}
	a.collectTypeNodeProvenance(surface, node.Type(), a.parameterReturnType(raw, subst), subst)
}

// apiStabilityMemberSymbolForDeclaration returns the member symbol a
// declaration contributes to an already resolved member table. A simply named
// declaration is looked up by its member-table key. A computed declaration is
// never asked for its name text: the table is scanned for the symbol whose
// declarations include the member, which keeps the pairing with the concrete
// instantiated member (and its signature) the table belongs to. The scan runs
// only for a computed declaration, so simply named members keep the direct
// lookup.
func apiStabilityMemberSymbolForDeclaration(members ast.SymbolTable, member *ast.Node) *ast.Symbol {
	if member == nil || members == nil {
		return nil
	}
	if name := ast.GetNameOfDeclaration(member); name != nil {
		if simple, ok := apiStabilitySimpleNameText(name); ok {
			return members[simple]
		}
	}
	for _, key := range sortedSymbolTableKeys(members) {
		candidate := members[key]
		if candidate == nil {
			continue
		}
		if slices.Contains(candidate.Declarations, member) {
			return candidate
		}
	}
	return nil
}

// collectTypeLiteralProvenance pairs a type literal's member annotations with
// the represented member types when the literal's member surface has been
// materialized. An unmaterialized member records nothing instead of being
// resolved. A computed member is paired by declaration identity, so its
// concrete instantiated member and signature are preserved without evaluating
// the computed name.
func (a *apiStabilityAnalysis) collectTypeLiteralProvenance(surface *apiStabilitySurface, node *ast.Node, represented *checker.Type, subst apiStabilitySubstitution) {
	c := a.tp.checker
	members, ok := c.GetResolvedMembersOfTypeIfMaterialized(represented)
	if !ok && represented != nil && represented.Symbol() != nil {
		members = a.materializedMembersOfSymbol(c, represented.Symbol())
		ok = members != nil
	}
	if !ok {
		return
	}
	for _, member := range node.Members() {
		if member == nil {
			continue
		}
		switch member.Kind {
		case ast.KindPropertySignature:
			propertySignature := member.AsPropertySignatureDeclaration()
			var propertyType *checker.Type
			if property := apiStabilityMemberSymbolForDeclaration(members, member); property != nil {
				// The concrete instantiated member is read through the ordinary
				// guarded accessor: a materialized component is used directly,
				// an unresolved one is materialized only when the guard
				// establishes that the read is bounded. This is the same lazy
				// path a simply named member takes through memberSurface.
				propertyType = a.typeOfSymbolSafely(property)
			}
			a.collectTypeNodeProvenance(surface, propertySignature.Type, propertyType, subst)
		case ast.KindMethodSignature:
			property := apiStabilityMemberSymbolForDeclaration(members, member)
			if property == nil {
				continue
			}
			if propertyType := a.typeOfSymbolSafely(property); propertyType != nil {
				if signatures, ok := c.GetResolvedSignaturesOfTypeIfMaterialized(propertyType, checker.SignatureKindCall); ok && len(signatures) != 0 {
					a.collectFunctionSignatureProvenance(surface, member, rawSignature(signatures[0]), subst)
					continue
				}
			}
			// A method's function type exists but its structured signatures have
			// not been materialized. The raw declaration signature is created
			// through the same guarded signature accessor an ordinary method
			// member uses.
			if !a.signatureMemberResolutionIsSafe(property, subst) {
				continue
			}
			if signatures := a.materializedSignaturesOfSymbol(c, property); len(signatures) != 0 {
				a.collectFunctionSignatureProvenance(surface, member, rawSignature(signatures[0]), subst)
			}
		case ast.KindCallSignature, ast.KindConstructSignature:
			kind := checker.SignatureKindCall
			if member.Kind == ast.KindConstructSignature {
				kind = checker.SignatureKindConstruct
			}
			if signatures, ok := c.GetResolvedSignaturesOfTypeIfMaterialized(represented, kind); ok && len(signatures) != 0 {
				a.collectFunctionSignatureProvenance(surface, member, rawSignature(signatures[0]), subst)
			}
		case ast.KindIndexSignature:
			if infos, ok := c.GetResolvedIndexInfosOfTypeIfMaterialized(represented); ok {
				for _, info := range infos {
					if info != nil && info.Declaration() == member {
						a.collectTypeNodeProvenance(surface, member.AsIndexSignatureDeclaration().Type, info.ValueType(), subst)
						break
					}
				}
			}
		}
	}
}

// collectFunctionSignatureProvenance pairs a function-like declaration's
// annotation nodes with the represented signature it declares.
func (a *apiStabilityAnalysis) collectFunctionSignatureProvenance(surface *apiStabilitySurface, declaration *ast.Node, raw *checker.Signature, subst apiStabilitySubstitution) {
	if declaration == nil || raw == nil {
		return
	}
	functionLike := declaration.FunctionLikeData()
	if functionLike == nil {
		return
	}
	rawParameters := raw.Parameters()
	if functionLike.Parameters != nil {
		parameterIndex := 0
		for _, parameter := range functionLike.Parameters.Nodes {
			if parameter == nil || parameter.Kind != ast.KindParameter {
				continue
			}
			parameterDeclaration := parameter.AsParameterDeclaration()
			if parameterDeclaration == nil {
				continue
			}
			parameterType := (*checker.Type)(nil)
			if parameterIndex < len(rawParameters) {
				parameterType = a.parameterTypeOf(rawParameters[parameterIndex])
			}
			a.collectTypeNodeProvenance(surface, parameterDeclaration.Type, parameterType, subst)
			parameterIndex++
		}
	}
	a.collectTypeNodeProvenance(surface, functionLike.Type, a.parameterReturnType(raw, subst), subst)
}

// representedTypeArguments returns the already recorded type arguments of a
// reference. A deferred reference whose arguments the checker has not
// materialized returns nil; nothing is resolved here.
func (a *apiStabilityAnalysis) representedTypeArguments(t *checker.Type) []*checker.Type {
	return a.tp.checker.GetResolvedTypeArguments(t)
}

// representedElementType returns the represented element type of an array or
// tuple reference.
func (a *apiStabilityAnalysis) representedElementType(t *checker.Type) *checker.Type {
	if arguments := a.representedTypeArguments(t); len(arguments) != 0 {
		return arguments[0]
	}
	return nil
}

// representedTupleArguments returns the represented element types of a tuple
// reference without instantiating anything.
func (a *apiStabilityAnalysis) representedTupleArguments(t *checker.Type) []*checker.Type {
	if t == nil || t.Flags()&checker.TypeFlagsObject == 0 || t.ObjectFlags()&checker.ObjectFlagsReference == 0 {
		return nil
	}
	return a.representedTypeArguments(t)
}

// recordSymbol records a symbol whose declared stability is above stable. A
// stable symbol can never exceed any owner ceiling, so it is omitted to keep
// each surface small.
func (a *apiStabilityAnalysis) recordSymbol(surface *apiStabilitySurface, symbol *ast.Symbol) {
	if symbol == nil {
		return
	}
	declared := a.tp.DeclaredApiStabilityOfSymbol(symbol)
	if declared.Level <= ApiStabilityStable {
		return
	}
	a.addFinding(surface, apiStabilityFindingKey{symbol: symbol}, declared)
}

// recordSignatureStability records a public signature whose declaration carries
// its own `@stability` tag. The raw (uninstantiated) signature keeps distinct
// overload tags distinct.
func (a *apiStabilityAnalysis) recordSignatureStability(surface *apiStabilitySurface, signature *checker.Signature) {
	if signature == nil {
		return
	}
	declared := a.tp.DeclaredApiStabilityOfSignature(signature)
	if declared.Level <= ApiStabilityStable {
		return
	}
	a.addFinding(surface, apiStabilityFindingKey{signature: rawSignature(signature)}, declared)
}

// typeChildBoundarySymbol returns the identity symbol whose explicit declared
// tag bounds a type reached as a child component, or nil when the type carries
// no explicit tag. An alias application's alias symbol takes precedence over
// the represented target, so an explicit tag on the alias is the component's
// own tag; otherwise the type's own symbol, or a reference's target symbol,
// carries the tag. An absent tag (Declaration nil) is not a boundary.
func (a *apiStabilityAnalysis) typeChildBoundarySymbol(t *checker.Type) *ast.Symbol {
	if t == nil {
		return nil
	}
	if alias := t.Alias(); alias != nil && alias.Symbol() != nil {
		if declared := a.tp.DeclaredApiStabilityOfSymbol(alias.Symbol()); declared.Declaration != nil {
			return alias.Symbol()
		}
	}
	symbol := t.Symbol()
	if symbol == nil && t.ObjectFlags()&checker.ObjectFlagsReference != 0 && t.Target() != nil {
		symbol = t.Target().Symbol()
	}
	if symbol == nil {
		return nil
	}
	if declared := a.tp.DeclaredApiStabilityOfSymbol(symbol); declared.Declaration != nil {
		return symbol
	}
	return nil
}

// heritageChildBoundarySymbol returns the identity symbol whose explicit
// declared tag bounds a heritage base reached as a child component, or nil.
// As with a type edge, an alias application's alias symbol takes precedence
// over the represented target.
func (a *apiStabilityAnalysis) heritageChildBoundarySymbol(base, target *checker.Type) *ast.Symbol {
	if base != nil {
		if alias := base.Alias(); alias != nil && alias.Symbol() != nil {
			if declared := a.tp.DeclaredApiStabilityOfSymbol(alias.Symbol()); declared.Declaration != nil {
				return alias.Symbol()
			}
		}
	}
	var symbol *ast.Symbol
	if target != nil {
		symbol = target.Symbol()
	}
	if symbol == nil && base != nil {
		symbol = base.Symbol()
	}
	if symbol == nil {
		return nil
	}
	if declared := a.tp.DeclaredApiStabilityOfSymbol(symbol); declared.Declaration != nil {
		return symbol
	}
	return nil
}

// collectChildTypeArguments merges the independently represented type arguments
// a tagged child type still exposes. Alias application arguments and reference
// arguments are operands of the application, not internals of the tagged
// component; each argument is itself traversed through the child boundary rules.
func (a *apiStabilityAnalysis) collectChildTypeArguments(surface *apiStabilitySurface, t *checker.Type, subst apiStabilitySubstitution) {
	if t == nil {
		return
	}
	if alias := t.Alias(); alias != nil && !a.aliasArgumentsAreSelf(alias) && apiStabilityRepresentedSurfaceAcceptsArguments(t) {
		for _, argument := range alias.TypeArguments() {
			if argument == nil {
				continue
			}
			surface.merge(a.typeSurface(argument, apiStabilityShallow, subst))
		}
	}
	if t.ObjectFlags()&checker.ObjectFlagsReference != 0 && t.Target() != nil && t.Target() != t {
		for _, argument := range a.referenceArguments(t, t.Target()) {
			if argument == nil {
				continue
			}
			surface.merge(a.typeSurface(argument, apiStabilityShallow, subst))
		}
	}
}

// collectTaggedHeritageArguments records every tagged class or interface
// ancestor of a declaration as a child boundary and exposes its independently
// represented type arguments. It is used where the compiler has already
// flattened inherited members into a resolved member table and no per-base
// declaration walk happens (an expanded reference). Untagged ancestors are
// followed so a tagged ancestor deeper in the chain is still attributed with
// the substitution context that reached it; an ancestor cycle is visited once.
func (a *apiStabilityAnalysis) collectTaggedHeritageArguments(surface *apiStabilitySurface, declaration *checker.Type, subst apiStabilitySubstitution, visited map[*checker.Type]bool) {
	if declaration == nil || visited[declaration] {
		return
	}
	visited[declaration] = true
	bases, resolved := a.tp.checker.GetResolvedBaseTypesOfTypeIfMaterialized(declaration)
	if !resolved {
		return
	}
	for _, base := range bases {
		if base == nil {
			continue
		}
		target := base
		if base.ObjectFlags()&checker.ObjectFlagsReference != 0 && base.Target() != nil {
			target = base.Target()
		}
		if target == nil || target.ObjectFlags()&checker.ObjectFlagsClassOrInterface == 0 {
			continue
		}
		if boundary := a.heritageChildBoundarySymbol(base, target); boundary != nil {
			a.recordSymbol(surface, boundary)
			a.collectChildTypeArguments(surface, base, subst)
			continue
		}
		baseSubst := a.extendParametersSubstitution(subst, base, a.referenceTypeParameters(target), a.referenceArguments(base, target))
		a.collectTaggedHeritageArguments(surface, target, baseSubst, visited)
	}
}

// declarationDeclaringSymbol returns the class or interface symbol whose
// declaration directly contains the given declaration, or nil for a
// declaration of a type literal or any other anonymous surface. Only a direct
// declaration names an owner: a declaration nested inside a type literal stays
// anonymous even when that literal sits inside a class or interface.
func (a *apiStabilityAnalysis) declarationDeclaringSymbol(declaration *ast.Node) *ast.Symbol {
	if declaration == nil || declaration.Parent == nil {
		return nil
	}
	switch declaration.Parent.Kind {
	case ast.KindClassDeclaration, ast.KindClassExpression, ast.KindInterfaceDeclaration:
		return checker.Checker_getSymbolOfDeclaration(a.tp.checker, declaration.Parent)
	}
	return nil
}

// memberDeclaringSymbol returns the class or interface symbol whose declaration
// directly contains the member, or nil for a member of a type literal or any
// other anonymous surface. Only a direct member declaration names an owner: a
// member nested inside a type literal stays anonymous even when that literal
// sits inside a class or interface.
func (a *apiStabilityAnalysis) memberDeclaringSymbol(member *ast.Symbol) *ast.Symbol {
	if member == nil {
		return nil
	}
	for _, declaration := range member.Declarations {
		if declaring := a.declarationDeclaringSymbol(declaration); declaring != nil {
			return declaring
		}
	}
	return nil
}

// inheritedChildBoundarySymbol returns the class or interface symbol that
// declares a member when the declaring symbol is not the member table's owner
// and carries an explicit declared tag. A member declared by the owner is an own
// member and is never gated; an untagged declaring symbol returns nil. This is
// how a member the compiler flattened into a derived member table from a tagged
// base is attributed to that base instead of being expanded as the derived
// component's own member. A same-named own override keeps the owner as its
// declaring symbol and stays inspected.
func (a *apiStabilityAnalysis) inheritedChildBoundarySymbol(member, owner *ast.Symbol) *ast.Symbol {
	if member == nil || owner == nil {
		return nil
	}
	declaring := a.memberDeclaringSymbol(member)
	if declaring == nil || declaring == owner {
		return nil
	}
	if declared := a.tp.DeclaredApiStabilityOfSymbol(declaring); declared.Declaration != nil {
		return declaring
	}
	return nil
}

// inheritedChildBoundarySymbolForDeclaration returns the class or interface
// symbol that directly declares a signature, constructor or index declaration
// when that declaring symbol is not the member table's owner and carries an
// explicit declared tag. The compiler flattens inherited call/construct
// signatures and index infos into a derived structured type while keeping each
// declaration's base provenance, so this attributes them to the tagged base
// exactly like inheritedChildBoundarySymbol does for flattened members. A
// declaration that belongs to the owner (an own call signature, own
// constructor, own index signature) is never gated, and a declaration without
// a class or interface parent (a type literal or a synthesized signature that
// has no declaration) returns nil without resolving anything.
func (a *apiStabilityAnalysis) inheritedChildBoundarySymbolForDeclaration(declaration *ast.Node, owner *ast.Symbol) *ast.Symbol {
	declaring := a.declarationDeclaringSymbol(declaration)
	if declaring == nil || declaring == owner {
		return nil
	}
	if declared := a.tp.DeclaredApiStabilityOfSymbol(declaring); declared.Declaration != nil {
		return declaring
	}
	return nil
}

// childSignatureBoundary records an explicitly tagged signature reached as a
// child component of owner and reports whether the signature is a boundary.
// A signature whose declaration belongs to the owner symbol is that owner's own
// signature (a root function's overload set) and stays walked; every other
// tagged signature contributes its declared level and its parameter, return and
// type-argument internals are not expanded. A signature the compiler flattened
// into a derived structured type keeps its declaration's base provenance, so a
// signature declared by a different, explicitly tagged class or interface is
// attributed to that tagged base (its declared level contributes and its
// internals are bounded). A synthesized default constructor signature without a
// declaration has no internals to expand and is never gated.
func (a *apiStabilityAnalysis) childSignatureBoundary(surface *apiStabilitySurface, signature *checker.Signature, owner *ast.Symbol) bool {
	raw := rawSignature(signature)
	if raw == nil {
		return false
	}
	if declared := a.tp.DeclaredApiStabilityOfSignature(raw); declared.Declaration != nil {
		if owner != nil && apiStabilityDeclarationBelongsToSymbol(declared.Declaration, owner) {
			return false
		}
		a.recordSignatureStability(surface, raw)
		return true
	}
	// A signature that is a declaration of the owner symbol itself (a method's
	// raw signature reached through its own function type) is the owner's own
	// exposed content, never a flattened inherited signature.
	if raw.Declaration() != nil && owner != nil && apiStabilityDeclarationBelongsToSymbol(raw.Declaration(), owner) {
		return false
	}
	if boundary := a.inheritedChildBoundarySymbolForDeclaration(raw.Declaration(), owner); boundary != nil {
		a.recordSymbol(surface, boundary)
		return true
	}
	return false
}

// declarationChildBoundary records an explicitly tagged declaration reached as
// a child component and reports whether the declaration's internals are bounded.
// It is used for index signatures, whose content annotations are the
// component's internals. A declaration whose own `@stability` tag is explicit
// contributes that tag. A declaration the compiler flattened into a derived
// structured type is attributed to its declaring class or interface exactly
// like a flattened signature: an explicitly tagged declaring symbol that is not
// the owner contributes its declared level and bounds the declaration's
// internals, so a tagged base's inherited index signature stays hidden while an
// own index signature (declaring symbol == owner) keeps being walked.
func (a *apiStabilityAnalysis) declarationChildBoundary(surface *apiStabilitySurface, declaration *ast.Node, owner *ast.Symbol) bool {
	if declaration == nil {
		return false
	}
	if declared := a.tp.DeclaredApiStabilityOfDeclaration(declaration); declared.Declaration != nil {
		a.recordDeclarationStability(surface, declaration)
		return true
	}
	if boundary := a.inheritedChildBoundarySymbolForDeclaration(declaration, owner); boundary != nil {
		a.recordSymbol(surface, boundary)
		return true
	}
	return false
}

// apiStabilityDeclarationBelongsToSymbol reports whether a declaration is one
// of a symbol's own declarations. It distinguishes a root function's overload
// signatures (declarations of the root symbol) from child signatures declared
// in another component.
func apiStabilityDeclarationBelongsToSymbol(declaration *ast.Node, symbol *ast.Symbol) bool {
	if declaration == nil || symbol == nil {
		return false
	}
	return slices.Contains(symbol.Declarations, declaration)
}

// recordDeclarationStability records a `@stability` tag carried by a
// declaration whose checker type has not been materialized. Declared member and
// signature stability contributes to the surface independently of type
// availability, so an annotation the checker has not resolved still reports its
// tagged members.
func (a *apiStabilityAnalysis) recordDeclarationStability(surface *apiStabilitySurface, declaration *ast.Node) {
	if declaration == nil {
		return
	}
	declared := a.tp.DeclaredApiStabilityOfDeclaration(declaration)
	if declared.Level <= ApiStabilityStable {
		return
	}
	a.addFinding(surface, apiStabilityFindingKey{declaration: declaration}, declared)
}

func (a *apiStabilityAnalysis) addFinding(surface *apiStabilitySurface, key apiStabilityFindingKey, declared ApiStabilityDeclaration) {
	existing, ok := surface.findings[key]
	if !ok || declared.Level > existing.level {
		surface.findings[key] = apiStabilityFinding{level: declared.Level, declaration: declared.Declaration}
	}
}

// declarationRepresentedType returns the represented type of a symbol's own
// declaration for annotation provenance. Type aliases use their declared type;
// values use the symbol's value type when it exists, otherwise the
// declaration's own annotation. An already computed component is preferred and
// a missing one is materialized lazily.
func (a *apiStabilityAnalysis) declarationRepresentedType(symbol *ast.Symbol, declaration *ast.Node) *checker.Type {
	c := a.tp.checker
	switch declaration.Kind {
	case ast.KindTypeAliasDeclaration, ast.KindJSTypeAliasDeclaration:
		if declared := a.declaredTypeOfSymbol(symbol); declared != nil {
			return declared
		}
		return a.representedTypeFromNode(declaration.Type(), apiStabilitySubstitution{})
	}
	if materialized := c.GetResolvedTypeOfSymbolIfMaterialized(symbol); materialized != nil {
		return materialized
	}
	switch declaration.Kind {
	case ast.KindVariableDeclaration:
		return a.representedTypeFromNode(declaration.AsVariableDeclaration().Type, apiStabilitySubstitution{})
	case ast.KindPropertyDeclaration:
		return a.representedTypeFromNode(declaration.AsPropertyDeclaration().Type, apiStabilitySubstitution{})
	case ast.KindPropertySignature:
		return a.representedTypeFromNode(declaration.AsPropertySignatureDeclaration().Type, apiStabilitySubstitution{})
	}
	return nil
}

// declarationAnnotationNode returns the public type annotation a declaration
// exposes for metadata reading, or nil.
func (a *apiStabilityAnalysis) declarationAnnotationNode(declaration *ast.Node) *ast.Node {
	if declaration == nil {
		return nil
	}
	if functionLike := declaration.FunctionLikeData(); functionLike != nil {
		return functionLike.Type
	}
	switch declaration.Kind {
	case ast.KindVariableDeclaration:
		return declaration.AsVariableDeclaration().Type
	case ast.KindPropertyDeclaration:
		return declaration.AsPropertyDeclaration().Type
	case ast.KindPropertySignature:
		return declaration.AsPropertySignatureDeclaration().Type
	case ast.KindTypeAliasDeclaration, ast.KindJSTypeAliasDeclaration:
		return declaration.Type()
	}
	return nil
}

// conditionalDeclarationIsDeferred reports whether a conditional written at a
// declaration site is symbolic: one of its operands mentions an unbound type
// parameter or an infer binder. The check is a narrow binder scan; it resolves
// nothing. A fully concrete conditional has a compiler outcome that the
// declaration does not represent and is never inspected through its branches.
func (a *apiStabilityAnalysis) conditionalDeclarationIsDeferred(node *ast.Node) bool {
	conditional := node.AsConditionalTypeNode()
	if conditional == nil {
		return false
	}
	visiting := make(map[*ast.Node]bool)
	return a.typeNodeMentionsTypeParameter(conditional.CheckType, visiting) ||
		a.typeNodeMentionsTypeParameter(conditional.ExtendsType, visiting)
}

// typeNodeMentionsTypeParameter reports whether a type node names an unbound
// type parameter (including an `infer` binder). It only looks up annotation
// symbols; no annotation type is resolved.
func (a *apiStabilityAnalysis) typeNodeMentionsTypeParameter(node *ast.Node, visiting map[*ast.Node]bool) bool {
	if node == nil || visiting[node] {
		return false
	}
	visiting[node] = true
	defer delete(visiting, node)
	switch node.Kind {
	case ast.KindInferType:
		return true
	case ast.KindTypeReference:
		reference := node.AsTypeReferenceNode()
		if symbol := a.symbolAtTypeNameNode(reference.TypeName); symbol != nil && symbol.Flags&ast.SymbolFlagsTypeParameter != 0 {
			return true
		}
		if reference.TypeArguments != nil {
			for _, argument := range reference.TypeArguments.Nodes {
				if a.typeNodeMentionsTypeParameter(argument, visiting) {
					return true
				}
			}
		}
	case ast.KindArrayType:
		return a.typeNodeMentionsTypeParameter(node.AsArrayTypeNode().ElementType, visiting)
	case ast.KindTupleType:
		if elements := node.AsTupleTypeNode().Elements; elements != nil {
			for _, element := range elements.Nodes {
				if a.typeNodeMentionsTypeParameter(element, visiting) {
					return true
				}
			}
		}
	case ast.KindNamedTupleMember:
		return a.typeNodeMentionsTypeParameter(node.AsNamedTupleMember().Type, visiting)
	case ast.KindOptionalType:
		return a.typeNodeMentionsTypeParameter(node.AsOptionalTypeNode().Type, visiting)
	case ast.KindRestType:
		return a.typeNodeMentionsTypeParameter(node.AsRestTypeNode().Type, visiting)
	case ast.KindParenthesizedType:
		return a.typeNodeMentionsTypeParameter(node.AsParenthesizedTypeNode().Type, visiting)
	case ast.KindTypeOperator:
		return a.typeNodeMentionsTypeParameter(node.AsTypeOperatorNode().Type, visiting)
	case ast.KindIndexedAccessType:
		indexed := node.AsIndexedAccessTypeNode()
		return a.typeNodeMentionsTypeParameter(indexed.ObjectType, visiting) ||
			a.typeNodeMentionsTypeParameter(indexed.IndexType, visiting)
	case ast.KindUnionType:
		if list := node.AsUnionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				if a.typeNodeMentionsTypeParameter(member, visiting) {
					return true
				}
			}
		}
	case ast.KindIntersectionType:
		if list := node.AsIntersectionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				if a.typeNodeMentionsTypeParameter(member, visiting) {
					return true
				}
			}
		}
	case ast.KindTemplateLiteralType:
		if template := node.AsTemplateLiteralTypeNode(); template != nil && template.TemplateSpans != nil {
			for _, span := range template.TemplateSpans.Nodes {
				if span == nil || span.Kind != ast.KindTemplateLiteralTypeSpan {
					continue
				}
				if a.typeNodeMentionsTypeParameter(span.AsTemplateLiteralTypeSpan().Type, visiting) {
					return true
				}
			}
		}
	case ast.KindConditionalType:
		conditional := node.AsConditionalTypeNode()
		return a.typeNodeMentionsTypeParameter(conditional.CheckType, visiting) ||
			a.typeNodeMentionsTypeParameter(conditional.ExtendsType, visiting)
	}
	return false
}

// shouldInspectSymbol reports whether a symbol has at least one declaration
// outside the default library. The standard library is never stability-tagged
// and expanding it would pull a large, irrelevant surface into every reference.
func (a *apiStabilityAnalysis) shouldInspectSymbol(symbol *ast.Symbol) bool {
	if symbol == nil || len(symbol.Declarations) == 0 {
		return false
	}
	for _, declaration := range symbol.Declarations {
		sourceFile := ast.GetSourceFileOfNode(declaration)
		if sourceFile == nil || !a.tp.program.IsSourceFileDefaultLibrary(ast.SourceFilePath(sourceFile)) {
			return true
		}
	}
	return false
}

// shouldInspectMemberDeclaration reports whether a member declaration's public
// annotation should be inspected for erased alias provenance. Function-like
// declarations are covered by their public signatures, except accessors whose
// value annotation is the only place an erased alias survives.
func (a *apiStabilityAnalysis) shouldInspectMemberDeclaration(declaration *ast.Node) bool {
	if declaration == nil {
		return false
	}
	if declaration.FunctionLikeData() == nil {
		return true
	}
	return declaration.Kind == ast.KindGetAccessor || declaration.Kind == ast.KindSetAccessor
}

// typeParameterAnnotationNode returns the declared constraint or default
// annotation of a type parameter.
func typeParameterAnnotationNode(t *checker.Type, constraint bool) *ast.Node {
	if t == nil {
		return nil
	}
	symbol := t.Symbol()
	if symbol == nil {
		return nil
	}
	for _, declaration := range symbol.Declarations {
		if declaration == nil || declaration.Kind != ast.KindTypeParameter {
			continue
		}
		typeParameterDeclaration := declaration.AsTypeParameterDeclaration()
		if typeParameterDeclaration == nil {
			continue
		}
		if constraint {
			return typeParameterDeclaration.Constraint
		}
		return typeParameterDeclaration.DefaultType
	}
	return nil
}

// signatureReturnTypeNode returns the declared return type annotation of a
// signature, or nil when the return type is inferred.
func signatureReturnTypeNode(signature *checker.Signature) *ast.Node {
	if signature == nil {
		return nil
	}
	declaration := signature.Declaration()
	if declaration == nil {
		return nil
	}
	functionLike := declaration.FunctionLikeData()
	if functionLike == nil {
		return nil
	}
	return functionLike.Type
}

// apiStabilityIndexSignatureKeyNode returns the key type node of an index
// signature declaration.
func apiStabilityIndexSignatureKeyNode(declaration *ast.Node) *ast.Node {
	if declaration == nil || declaration.Kind != ast.KindIndexSignature {
		return nil
	}
	parameters := declaration.Parameters()
	if len(parameters) != 1 || parameters[0] == nil || parameters[0].Kind != ast.KindParameter {
		return nil
	}
	return parameters[0].AsParameterDeclaration().Type
}

// apiStabilityIndexSignatureValueNode returns the value type node of an index
// signature declaration.
func apiStabilityIndexSignatureValueNode(declaration *ast.Node) *ast.Node {
	if declaration == nil || declaration.Kind != ast.KindIndexSignature {
		return nil
	}
	return declaration.AsIndexSignatureDeclaration().Type
}

// apiStabilityMappedDeclarationHasName reports whether a mapped type declared a
// `as` clause whose name type the checker has not materialized.
func apiStabilityMappedDeclarationHasName(t *checker.Type) bool {
	if t == nil {
		return false
	}
	symbol := t.Symbol()
	if symbol == nil {
		return false
	}
	for _, declaration := range symbol.Declarations {
		if declaration == nil || declaration.Kind != ast.KindMappedType {
			continue
		}
		if mapped := declaration.AsMappedTypeNode(); mapped != nil && mapped.NameType != nil {
			return true
		}
	}
	return false
}

// apiStabilityTypeAliasDeclaration returns the type alias declaration of a
// symbol.
func apiStabilityTypeAliasDeclaration(symbol *ast.Symbol) *ast.Node {
	if symbol == nil {
		return nil
	}
	for _, declaration := range symbol.Declarations {
		if declaration != nil && (declaration.Kind == ast.KindTypeAliasDeclaration || declaration.Kind == ast.KindJSTypeAliasDeclaration) {
			return declaration
		}
	}
	return nil
}

// apiStabilitySymbolDeclaresHeritage reports whether a class or interface
// symbol declares heritage clauses. It reads declaration structure only and is
// used to distinguish an unmaterialized heritage surface from one with nothing
// to resolve.
func apiStabilitySymbolDeclaresHeritage(symbol *ast.Symbol) bool {
	if symbol == nil {
		return false
	}
	for _, declaration := range symbol.Declarations {
		if declaration == nil {
			continue
		}
		var clauses *ast.HeritageClauseList
		switch declaration.Kind {
		case ast.KindClassDeclaration, ast.KindClassExpression:
			if classLike := declaration.ClassLikeData(); classLike != nil {
				clauses = classLike.HeritageClauses
			}
		case ast.KindInterfaceDeclaration:
			clauses = declaration.AsInterfaceDeclaration().HeritageClauses
		}
		if clauses == nil {
			continue
		}
		for _, clauseNode := range clauses.Nodes {
			if clauseNode == nil {
				continue
			}
			clause := clauseNode.AsHeritageClause()
			if clause != nil && clause.Types != nil && len(clause.Types.Nodes) != 0 {
				return true
			}
		}
	}
	return false
}

func sortedSymbolTableKeys(table ast.SymbolTable) []string {
	keys := make([]string, 0, len(table))
	for key := range table {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

// apiStabilityMemberTableNameIsVisible reports whether a member-table entry is
// a candidate public member. Entries whose names use the checker's documented
// internal symbol-name prefix are reserved implementation spellings (`__call`,
// `__new`, `__index`, `__export`, the binder's `__computed` placeholder, ambient
// module patterns, ...) and are never public members. A late-bound computed
// member is the exception: the checker names a computed declaration by the
// property name its literal or unique-symbol value resolves to, and
// `checker.IsKnownSymbol` identifies exactly those spellings. Its accessibility
// is still decided by its declaration provenance (`apiStabilitySymbolIsNonPublic`)
// at the call site, and an unresolved dynamic name keeps the `__computed`
// placeholder, which stays invisible.
func apiStabilityMemberTableNameIsVisible(name string, member *ast.Symbol) bool {
	if member == nil {
		return false
	}
	if !strings.HasPrefix(name, ast.InternalSymbolNamePrefix) {
		return true
	}
	return checker.IsKnownSymbol(member)
}

// apiStabilitySymbolIsNonPublic classifies a member symbol by its declaration
// accessibility. Private identifiers and private/protected members are excluded;
// a leading `__` name is not treated as private.
func apiStabilitySymbolIsNonPublic(symbol *ast.Symbol) bool {
	if symbol == nil {
		return true
	}
	if checker.IsPrivateIdentifierSymbol(symbol) {
		return true
	}
	flags := checker.GetDeclarationModifierFlagsFromSymbol(symbol)
	return flags&(ast.ModifierFlagsPrivate|ast.ModifierFlagsProtected) != 0
}

// apiStabilitySignatureIsNonPublic filters construct and method signatures whose
// declaration is private or protected.
func apiStabilitySignatureIsNonPublic(signature *checker.Signature) bool {
	declaration := signature.Declaration()
	if declaration == nil {
		return false
	}
	if name := ast.GetNameOfDeclaration(declaration); name != nil && ast.IsPrivateIdentifier(name) {
		return true
	}
	flags := ast.GetCombinedModifierFlags(declaration)
	return flags&(ast.ModifierFlagsPrivate|ast.ModifierFlagsProtected) != 0
}
