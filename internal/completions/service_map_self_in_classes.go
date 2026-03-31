package completions

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/completion"
	"github.com/effect-ts/tsgo/internal/keybuilder"
	"github.com/effect-ts/tsgo/internal/pluginoptions"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/tspath"
)

// serviceMapSelfInClasses provides completion items for ServiceMap.Service class constructors
// when the cursor is in the extends clause of a class declaration.
// This is a V4-only completion.
var serviceMapSelfInClasses = completion.Completion{
	Name:        "serviceMapSelfInClasses",
	Description: "Provides ServiceMap.Service completions in extends clauses",
	Run:         runServiceMapSelfInClasses,
}

func runServiceMapSelfInClasses(ctx *completion.Context) []*lsproto.CompletionItem {
	data := completion.ParseDataForExtendsClassCompletion(ctx.SourceFile, ctx.Position)
	if data == nil {
		return nil
	}

	ch := ctx.Checker
	tp := ctx.TypeParser
	// V4 only
	version := tp.SupportedEffectVersion()
	if version != typeparser.EffectMajorV4 {
		return nil
	}

	serviceMapIdentifier := typeparser.FindModuleIdentifier(ctx.SourceFile, "ServiceMap")
	accessedText := data.AccessedObjectText()
	isFullyQualified := serviceMapIdentifier == accessedText
	className := data.ClassNameText()

	// For non-fully-qualified: validate with IsNodeReferenceToServiceMapModuleApi
	if !isFullyQualified && !tp.IsNodeReferenceToServiceMapModuleApi(data.AccessedObject, "Service") {
		return nil
	}

	// Compute deterministic tag key
	tagKey := computeServiceTagKey(ctx.Program, tp, ch, ctx.SourceFile, className)

	// Build replacement range from byte offsets
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)

	sortText := "11"
	var items []*lsproto.CompletionItem

	// Service<ClassName, {}> — inline type parameters
	{
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Service<%s, {${0}}>()("%s"){}`, serviceMapIdentifier, className, tagKey)
		} else {
			insertText = fmt.Sprintf(`Service<%s, {${0}}>()("%s"){}`, className, tagKey)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf("Service<%s, {}>", className),
			insertText, sortText, replacementRange,
		))
	}

	// Service<ClassName>({ make }) — factory-based construction
	{
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Service<%s>()("%s", { make: ${0} }){}`, serviceMapIdentifier, className, tagKey)
		} else {
			insertText = fmt.Sprintf(`Service<%s>()("%s", { make: ${0} }){}`, className, tagKey)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf("Service<%s>({ make })", className),
			insertText, sortText, replacementRange,
		))
	}

	return items
}

// computeServiceTagKey computes the deterministic tag key for a service class.
// Falls back to the class name if keybuilder returns empty.
func computeServiceTagKey(program checker.Program, tp typeparser.TypeParser, ch *checker.Checker, sf *ast.SourceFile, className string) string {
	pkgJson := tp.PackageJsonForSourceFile(sf)
	if pkgJson == nil {
		return className
	}
	packageName, ok := pkgJson.Name.GetValue()
	if !ok || packageName == "" {
		return className
	}

	packageDirectory := getCompletionPackageJsonDirectory(program, ch, sf)
	if packageDirectory == "" {
		return className
	}

	effectConfig := program.Options().Effect
	if effectConfig == nil {
		return className
	}
	resolvedOptions := pluginoptions.ResolveEffectPluginOptionsForSourceFile(
		effectConfig,
		sf.FileName(),
		program.Options().ConfigFilePath,
		program.UseCaseSensitiveFileNames(),
	)
	keyPatterns := resolvedOptions.GetKeyPatterns()

	key := keybuilder.CreateString(sf.FileName(), packageName, packageDirectory, className, "service", keyPatterns)
	if key == "" {
		return className
	}
	return key
}

// getCompletionPackageJsonDirectory gets the package.json directory for a source file.
func getCompletionPackageJsonDirectory(program checker.Program, c *checker.Checker, sf *ast.SourceFile) string {
	type metaProvider interface {
		GetSourceFileMetaData(path tspath.Path) ast.SourceFileMetaData
	}

	prog, ok := program.(metaProvider)
	if !ok || prog == nil {
		return ""
	}

	meta := prog.GetSourceFileMetaData(sf.Path())
	return meta.PackageJsonDirectory
}
