package rules

import (
	"strings"

	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/keybuilder"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/microsoft/typescript-go/shim/tspath"
)

// DeterministicKeys ensures string key literals in Effect service/tag/error constructors
// follow a deterministic, location-based naming convention.
var DeterministicKeys = rule.Rule{
	Name:            "deterministicKeys",
	Description:     "Enforces deterministic naming for service/tag/error identifiers based on class names",
	DefaultSeverity: etscore.SeverityOff,
	Codes:           []int32{tsdiag.Key_should_be_0_effect_deterministicKeys.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeDeterministicKeys(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.Location, tsdiag.Key_should_be_0_effect_deterministicKeys, nil, m.ExpectedKey)
		}
		return diags
	},
}

// DeterministicKeyMatch holds the AST nodes and computed key info needed by both
// the diagnostic rule and the quick-fix for the deterministicKeys pattern.
type DeterministicKeyMatch struct {
	Location         core.TextRange // The pre-computed error range for the key string literal
	KeyStringLiteral *ast.Node      // The key string literal node
	ActualKey        string         // The actual key string found in the source
	ExpectedKey      string         // The expected key computed by keybuilder
}

// AnalyzeDeterministicKeys finds all class declarations where the key string literal
// doesn't match the expected deterministic key.
func AnalyzeDeterministicKeys(c *checker.Checker, sf *ast.SourceFile) []DeterministicKeyMatch {
	effectConfig := getEffectConfig(c.Program())
	if effectConfig == nil {
		return nil
	}

	keyPatterns := effectConfig.GetKeyPatterns()
	extendedKeyDetection := effectConfig.ExtendedKeyDetection

	var matches []DeterministicKeyMatch

	nodeToVisit := make([]*ast.Node, 0)
	pushChild := func(child *ast.Node) bool {
		nodeToVisit = append(nodeToVisit, child)
		return false
	}
	sf.AsNode().ForEachChild(pushChild)

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		if node.Kind == ast.KindClassDeclaration && node.Name() != nil {
			if m := checkDeterministicKeyMatch(c, sf, node, keyPatterns, extendedKeyDetection); m != nil {
				matches = append(matches, *m)
			}
		}

		node.ForEachChild(pushChild)
	}

	return matches
}

// getEffectConfig retrieves the Effect plugin configuration from the program's compiler options.
func getEffectConfig(p checker.Program) *etscore.EffectPluginOptions {
	return p.Options().Effect
}

// deterministicKeyMatch holds the matched key info from a class declaration.
type deterministicKeyMatch struct {
	className        *ast.Node
	keyStringLiteral *ast.Node
	target           string
}

func checkDeterministicKeyMatch(c *checker.Checker, sf *ast.SourceFile, classNode *ast.Node, keyPatterns []etscore.KeyPattern, extendedKeyDetection bool) *DeterministicKeyMatch {
	match := matchClassPattern(c, sf, classNode, extendedKeyDetection)
	if match == nil || match.keyStringLiteral == nil {
		return nil
	}

	// Get class name text
	classNameText := scanner.GetTextOfNode(match.className)

	// Get package info
	pkgJson := typeparser.PackageJsonForSourceFile(c, sf)
	if pkgJson == nil {
		return nil
	}
	packageName, ok := pkgJson.Name.GetValue()
	if !ok || packageName == "" {
		return nil
	}

	// Get package directory from source file metadata
	packageDirectory := getPackageJsonDirectory(c, sf)
	if packageDirectory == "" {
		return nil
	}

	// Get source file name
	sourceFileName := sf.FileName()

	// Compute expected key
	expectedKey := keybuilder.CreateString(sourceFileName, packageName, packageDirectory, classNameText, match.target, keyPatterns)
	if expectedKey == "" {
		return nil
	}

	// Get actual key
	actualKey := match.keyStringLiteral.AsStringLiteral().Text

	if actualKey == expectedKey {
		return nil
	}

	return &DeterministicKeyMatch{
		Location:         scanner.GetErrorRangeForNode(sf, match.keyStringLiteral),
		KeyStringLiteral: match.keyStringLiteral,
		ActualKey:        actualKey,
		ExpectedKey:      expectedKey,
	}
}

// matchClassPattern tries to match a class declaration against the supported patterns.
// Priority: service targets first, then error targets, then custom.
func matchClassPattern(c *checker.Checker, sf *ast.SourceFile, classNode *ast.Node, extendedKeyDetection bool) *deterministicKeyMatch {
	// Service target: ExtendsEffectService → ExtendsContextTag → ExtendsEffectTag → ExtendsServiceMapService
	if result := typeparser.ExtendsEffectService(c, classNode); result != nil {
		return &deterministicKeyMatch{className: result.ClassName, keyStringLiteral: result.KeyStringLiteral, target: "service"}
	}
	if result := typeparser.ExtendsContextTag(c, classNode); result != nil {
		return &deterministicKeyMatch{className: result.ClassName, keyStringLiteral: result.KeyStringLiteral, target: "service"}
	}
	if result := typeparser.ExtendsEffectTag(c, classNode); result != nil {
		return &deterministicKeyMatch{className: result.ClassName, keyStringLiteral: result.KeyStringLiteral, target: "service"}
	}
	if result := typeparser.ExtendsServiceMapService(c, classNode); result != nil {
		return &deterministicKeyMatch{className: result.ClassName, keyStringLiteral: result.KeyStringLiteral, target: "service"}
	}

	// Error target: ExtendsDataTaggedError → ExtendsSchemaTaggedError
	if result := typeparser.ExtendsDataTaggedError(c, classNode); result != nil {
		return &deterministicKeyMatch{className: result.ClassName, keyStringLiteral: result.KeyStringLiteral, target: "error"}
	}
	if result := typeparser.ExtendsSchemaTaggedError(c, classNode); result != nil {
		return &deterministicKeyMatch{className: result.ClassName, keyStringLiteral: result.KeyStringLiteral, target: "error"}
	}

	// Custom target (only if extendedKeyDetection is enabled)
	if extendedKeyDetection {
		if result := matchCustomPattern(c, sf, classNode); result != nil {
			return result
		}
	}

	return nil
}

// matchCustomPattern checks heritage clause nodes for call expressions with string literal
// arguments whose parameter declarations contain the @effect-identifier annotation.
func matchCustomPattern(c *checker.Checker, sf *ast.SourceFile, classNode *ast.Node) *deterministicKeyMatch {
	if classNode.Name() == nil {
		return nil
	}

	heritageClauses := classNode.ClassLikeData().HeritageClauses
	if heritageClauses == nil {
		return nil
	}

	// BFS through heritage clause nodes
	nodesToVisit := make([]*ast.Node, 0)
	nodesToVisit = append(nodesToVisit, heritageClauses.Nodes...)
	pushHeritageChild := func(child *ast.Node) bool {
		nodesToVisit = append(nodesToVisit, child)
		return false
	}

	for len(nodesToVisit) > 0 {
		current := nodesToVisit[0]
		nodesToVisit = nodesToVisit[1:]

		if ast.IsCallExpression(current) {
			call := current.AsCallExpression()
			if call.Arguments != nil {
				for i, arg := range call.Arguments.Nodes {
					if !ast.IsStringLiteral(arg) {
						continue
					}

					sig := c.GetResolvedSignature(current)
					if sig == nil {
						continue
					}

					params := sig.Parameters()
					if i >= len(params) {
						continue
					}

					param := params[i]
					if param.Declarations == nil {
						continue
					}

					for _, decl := range param.Declarations {
						paramSf := ast.GetSourceFileOfNode(decl)
						if paramSf == nil {
							continue
						}
						paramText := paramSf.Text()
						pos := decl.Pos()
						end := decl.End()
						if pos >= 0 && end >= pos && end <= len(paramText) {
							declText := paramText[pos:end]
							if strings.Contains(strings.ToLower(declText), "@effect-identifier") {
								return &deterministicKeyMatch{
									className:        classNode.Name(),
									keyStringLiteral: arg,
									target:           "custom",
								}
							}
						}
					}
				}
			}
		}

		// Visit children
		current.ForEachChild(pushHeritageChild)
	}

	return nil
}

// getPackageJsonDirectory gets the package.json directory for a source file from its metadata.
func getPackageJsonDirectory(c *checker.Checker, sf *ast.SourceFile) string {
	type metaProvider interface {
		GetSourceFileMetaData(path tspath.Path) ast.SourceFileMetaData
	}

	prog, ok := c.Program().(metaProvider)
	if !ok || prog == nil {
		return ""
	}

	meta := prog.GetSourceFileMetaData(sf.Path())
	return meta.PackageJsonDirectory
}
