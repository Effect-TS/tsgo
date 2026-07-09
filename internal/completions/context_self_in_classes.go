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

// contextSelfInClasses provides completion items for Effect context/service class
// constructors when the cursor is in the extends clause of a class declaration.
var contextSelfInClasses = completion.Completion{
	Name:        "contextSelfInClasses",
	Description: "Provides Context.Tag / Context.Service completions in extends clauses",
	Run:         runContextSelfInClasses,
}

func runContextSelfInClasses(ctx *completion.Context) []*lsproto.CompletionItem {
	data := completion.ParseDataForExtendsClassCompletion(ctx.SourceFile, ctx.Position)
	if data == nil {
		return nil
	}

	ch := ctx.Checker
	tp := ctx.TypeParser
	version := tp.SupportedEffectVersion()
	className := data.ClassNameText()
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)
	sortText := "11"
	tagKey := computeServiceTagKey(ctx.Program, tp, ch, ctx.SourceFile, className)

	contextIdentifier := typeparser.FindModuleIdentifier(ctx.SourceFile, "Context")
	accessedText := data.AccessedObjectText()
	isFullyQualified := contextIdentifier == accessedText

	if version == typeparser.EffectMajorV3 {
		if !isFullyQualified && !tp.IsNodeReferenceToEffectContextModuleApi(data.AccessedObject, "Tag") {
			return nil
		}

		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Tag("%s")<%s, ${0}>(){}`, contextIdentifier, tagKey, className)
		} else {
			insertText = fmt.Sprintf(`Tag("%s")<%s, ${0}>(){}`, tagKey, className)
		}

		return []*lsproto.CompletionItem{
			makeExtendsCompletionItem(accessedText,
				fmt.Sprintf(`Tag("%s")`, className),
				insertText, sortText, replacementRange,
			),
		}
	}

	if !isFullyQualified && !tp.IsNodeReferenceToEffectContextModuleApi(data.AccessedObject, "Service") {
		return nil
	}

	var items []*lsproto.CompletionItem
	{
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Service<%s, {${0}}>()("%s"){}`, contextIdentifier, className, tagKey)
		} else {
			insertText = fmt.Sprintf(`Service<%s, {${0}}>()("%s"){}`, className, tagKey)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf("Service<%s, {}>", className),
			insertText, sortText, replacementRange,
		))
	}
	{
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Service<%s>()("%s", { make: ${0} }){}`, contextIdentifier, className, tagKey)
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
func computeServiceTagKey(program checker.Program, tp *typeparser.TypeParser, ch *checker.Checker, sf *ast.SourceFile, className string) string {
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
func getCompletionPackageJsonDirectory(program checker.Program, _ *checker.Checker, sf *ast.SourceFile) string {
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
