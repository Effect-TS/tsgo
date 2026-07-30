package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"go/ast"
	"go/constant"
	"go/token"
	"go/types"
	"log"
	"os"
	"path/filepath"
	"reflect"
	"slices"
	"strconv"
	"strings"

	"golang.org/x/tools/go/packages"
)

const (
	protocolPackage   = "github.com/effect-ts/tsgo/etsjsapi"
	effectCorePackage = "github.com/effect-ts/tsgo/etscore"
	outputPath        = "_packages/tsgo/src/experimental/api/protocol.generated.ts"
)

type protocolMethod struct {
	name   string
	params types.Type
	result types.Type
}

type generator struct {
	pkg   *packages.Package
	types map[string]*types.Named
}

func main() {
	pkg := loadProtocolPackage()
	g := &generator{pkg: pkg, types: make(map[string]*types.Named)}
	methods, err := g.protocolMethods()
	if err != nil {
		log.Fatal(err)
	}
	for _, method := range methods {
		if err := g.collect(method.params); err != nil {
			log.Fatal(err)
		}
		if err := g.collect(method.result); err != nil {
			log.Fatal(err)
		}
	}
	output, err := g.generate(methods)
	if err != nil {
		log.Fatal(err)
	}
	repositoryRoot := filepath.Dir(filepath.Dir(pkg.GoFiles[0]))
	if err := os.WriteFile(filepath.Join(repositoryRoot, outputPath), output, 0o644); err != nil {
		log.Fatal(err)
	}
}

func loadProtocolPackage() *packages.Package {
	loaded, err := packages.Load(&packages.Config{
		Mode: packages.NeedName | packages.NeedFiles | packages.NeedTypes | packages.NeedSyntax | packages.NeedTypesInfo,
	}, protocolPackage)
	if err != nil {
		log.Fatal(err)
	}
	if packages.PrintErrors(loaded) != 0 || len(loaded) != 1 {
		log.Fatalf("failed to load %s", protocolPackage)
	}
	return loaded[0]
}

func (g *generator) protocolMethods() ([]protocolMethod, error) {
	names := make(map[*types.Var]string)
	for _, file := range g.pkg.Syntax {
		ast.Inspect(file, func(node ast.Node) bool {
			declaration, ok := node.(*ast.GenDecl)
			if !ok || declaration.Tok != token.VAR {
				return true
			}
			for _, spec := range declaration.Specs {
				valueSpec := spec.(*ast.ValueSpec)
				for index, name := range valueSpec.Names {
					variable, ok := g.pkg.TypesInfo.Defs[name].(*types.Var)
					if !ok || index >= len(valueSpec.Values) {
						continue
					}
					literal, ok := valueSpec.Values[index].(*ast.CompositeLit)
					if !ok {
						continue
					}
					for _, element := range literal.Elts {
						field, ok := element.(*ast.KeyValueExpr)
						if !ok {
							continue
						}
						key, ok := field.Key.(*ast.Ident)
						if !ok || key.Name != "Name" {
							continue
						}
						value := g.pkg.TypesInfo.Types[field.Value].Value
						if value != nil && value.Kind() == constant.String {
							names[variable] = constant.StringVal(value)
						}
					}
				}
			}
			return false
		})
	}

	var methods []protocolMethod
	for _, name := range g.pkg.Types.Scope().Names() {
		variable, ok := g.pkg.Types.Scope().Lookup(name).(*types.Var)
		if !ok {
			continue
		}
		named, ok := variable.Type().(*types.Named)
		if !ok || named.Origin().Obj().Name() != "method" || named.TypeArgs().Len() != 2 {
			continue
		}
		methodName := names[variable]
		if methodName == "" {
			return nil, fmt.Errorf("protocol method %s has no constant Name", name)
		}
		methods = append(methods, protocolMethod{
			name:   methodName,
			params: named.TypeArgs().At(0),
			result: named.TypeArgs().At(1),
		})
	}
	slices.SortFunc(methods, func(a, b protocolMethod) int { return strings.Compare(a.name, b.name) })
	if len(methods) == 0 {
		return nil, fmt.Errorf("no protocol methods found")
	}
	for index := 1; index < len(methods); index++ {
		if methods[index-1].name == methods[index].name {
			return nil, fmt.Errorf("duplicate protocol method %q", methods[index].name)
		}
	}
	for _, method := range methods {
		result, ok := method.result.(*types.Named)
		if !ok {
			return nil, fmt.Errorf("protocol method %q result must be a named struct", method.name)
		}
		if _, ok := result.Underlying().(*types.Struct); !ok {
			return nil, fmt.Errorf("protocol method %q result must be a named struct", method.name)
		}
	}
	return methods, nil
}

func (g *generator) collect(value types.Type) error {
	switch value := value.(type) {
	case *types.Named:
		if value.TypeArgs().Len() != 0 {
			return fmt.Errorf("generic protocol payload type %s is not supported", value.String())
		}
		if value.Obj().Pkg() == nil || !isProtocolTypePackage(value.Obj().Pkg().Path()) {
			return fmt.Errorf("unsupported named type %s", value.String())
		}
		name := value.Obj().Name()
		if existing, exists := g.types[name]; exists {
			if !types.Identical(existing, value) {
				return fmt.Errorf("protocol types %s and %s have the same TypeScript name", existing.String(), value.String())
			}
			return nil
		}
		g.types[name] = value
		return g.collect(value.Underlying())
	case *types.Struct:
		for index := range value.NumFields() {
			field := value.Field(index)
			if !field.Exported() {
				continue
			}
			if field.Anonymous() {
				return fmt.Errorf("anonymous protocol field %s is not supported", field.Name())
			}
			_, _, include, err := jsonField(field.Name(), value.Tag(index))
			if err != nil {
				return err
			}
			if include {
				if err := g.collect(field.Type()); err != nil {
					return err
				}
			}
		}
		return nil
	case *types.Slice:
		if basic, ok := value.Elem().(*types.Basic); ok && basic.Kind() == types.Byte {
			return fmt.Errorf("[]byte protocol fields are not supported because encoding/json emits base64 strings")
		}
		return g.collect(value.Elem())
	case *types.Pointer:
		return g.collect(value.Elem())
	case *types.Basic:
		return validateBasic(value)
	case *types.Map:
		key, ok := value.Key().(*types.Basic)
		if !ok || key.Kind() != types.String {
			return fmt.Errorf("protocol map keys must be strings, received %s", value.Key().String())
		}
		return g.collect(value.Elem())
	case *types.Interface:
		if value.Empty() {
			return nil
		}
	}
	return fmt.Errorf("unsupported protocol type %T (%s)", value, value.String())
}

func (g *generator) generate(methods []protocolMethod) ([]byte, error) {
	versionObject := g.pkg.Types.Scope().Lookup("ProtocolVersion")
	version, ok := versionObject.(*types.Const)
	if !ok {
		return nil, fmt.Errorf("ProtocolVersion must be a constant")
	}

	var output bytes.Buffer
	output.WriteString("// Code generated by go run ./_tools/gen_etsjsapi; DO NOT EDIT.\n\n")
	fmt.Fprintf(&output, "export const protocolVersion = %s as const\n\n", version.Val().ExactString())

	names := make([]string, 0, len(g.types))
	for name := range g.types {
		names = append(names, name)
	}
	slices.Sort(names)
	for _, name := range names {
		if err := g.writeNamedType(&output, g.types[name]); err != nil {
			return nil, err
		}
		output.WriteByte('\n')
	}

	output.WriteString("export interface MethodMap {\n")
	for _, method := range methods {
		fmt.Fprintf(&output, "  readonly %s: {\n", quoteProperty(method.name))
		fmt.Fprintf(&output, "    readonly params: %s\n", g.typeScriptType(method.params))
		fmt.Fprintf(&output, "    readonly result: %s\n", g.typeScriptType(method.result))
		output.WriteString("  }\n")
	}
	output.WriteString("}\n\n")
	output.WriteString("export type Method = keyof MethodMap\n\n")
	output.WriteString("export interface WireResponse<M extends Method> {\n")
	output.WriteString("  readonly version: number\n")
	output.WriteString("  readonly id: number\n")
	output.WriteString("  readonly result?: MethodMap[M][\"result\"]\n")
	output.WriteString("  readonly error?: { readonly message: string }\n")
	output.WriteString("}\n")
	return output.Bytes(), nil
}

func (g *generator) writeNamedType(output *bytes.Buffer, named *types.Named) error {
	name := named.Obj().Name()
	switch underlying := named.Underlying().(type) {
	case *types.Struct:
		fmt.Fprintf(output, "export interface %s {\n", name)
		for index := range underlying.NumFields() {
			field := underlying.Field(index)
			if !field.Exported() {
				continue
			}
			jsonName, optional, include, err := jsonField(field.Name(), underlying.Tag(index))
			if err != nil {
				return err
			}
			if !include {
				continue
			}
			marker := ""
			if named.Obj().Pkg().Path() == effectCorePackage {
				optional = true
			}
			if optional {
				marker = "?"
			}
			fieldType, err := g.typeScriptFieldType(field.Type(), underlying.Tag(index))
			if err != nil {
				return err
			}
			fmt.Fprintf(output, "  readonly %s%s: %s\n", quoteProperty(jsonName), marker, fieldType)
		}
		output.WriteString("}\n")
		return nil
	case *types.Basic:
		if named.Obj().Pkg().Path() == effectCorePackage && name == "Severity" {
			output.WriteString("export type Severity = \"off\" | \"warning\" | \"warn\" | \"error\" | \"suggestion\" | \"message\" | \"skip-file\"\n")
			return nil
		}
		constants := g.constantsOfType(named)
		if len(constants) != 0 {
			fmt.Fprintf(output, "export type %s = %s\n", name, strings.Join(constants, " | "))
			return nil
		}
		fmt.Fprintf(output, "export type %s = %s\n", name, g.typeScriptType(underlying))
		return nil
	default:
		return fmt.Errorf("unsupported named protocol type %s", named.String())
	}
}

func (g *generator) constantsOfType(named *types.Named) []string {
	var values []string
	scope := named.Obj().Pkg().Scope()
	for _, name := range scope.Names() {
		value, ok := scope.Lookup(name).(*types.Const)
		if !ok || !types.Identical(value.Type(), named) {
			continue
		}
		values = append(values, value.Val().ExactString())
	}
	slices.Sort(values)
	return values
}

func (g *generator) typeScriptType(value types.Type) string {
	switch value := value.(type) {
	case *types.Named:
		return value.Obj().Name()
	case *types.Slice:
		return "ReadonlyArray<" + g.typeScriptType(value.Elem()) + ">"
	case *types.Pointer:
		return g.typeScriptType(value.Elem())
	case *types.Map:
		return "Readonly<Record<string, " + g.typeScriptType(value.Elem()) + ">>"
	case *types.Interface:
		return "unknown"
	case *types.Basic:
		switch value.Kind() {
		case types.String:
			return "string"
		case types.Bool:
			return "boolean"
		case types.Int, types.Int8, types.Int16, types.Int32,
			types.Uint, types.Uint8, types.Uint16, types.Uint32,
			types.Float32, types.Float64:
			return "number"
		}
	}
	panic(fmt.Sprintf("unsupported protocol type %T (%s)", value, value.String()))
}

func (g *generator) typeScriptFieldType(value types.Type, tag string) (string, error) {
	originalValue := value
	var rendered string
	if enum := reflect.StructTag(tag).Get("schema_enum"); enum != "" {
		parsed, err := stringEnum(enum)
		if err != nil {
			return "", err
		}
		rendered = parsed
	} else if enum := reflect.StructTag(tag).Get("schema_items_enum"); enum != "" {
		itemType, err := stringEnum(enum)
		if err != nil {
			return "", err
		}
		if pointer, ok := value.(*types.Pointer); ok {
			value = pointer.Elem()
		}
		if _, ok := value.Underlying().(*types.Slice); !ok {
			return "", fmt.Errorf("schema_items_enum requires a slice, received %s", value.String())
		}
		rendered = "ReadonlyArray<" + itemType + ">"
	} else {
		rendered = g.typeScriptType(value)
	}

	switch reflect.StructTag(tag).Get("etsjsapi") {
	case "":
		return rendered, nil
	case "nullable":
		if _, ok := originalValue.(*types.Pointer); !ok {
			return "", fmt.Errorf("nullable protocol field must be a pointer, received %s", originalValue.String())
		}
		return rendered + " | null", nil
	default:
		return "", fmt.Errorf("unsupported etsjsapi field option %q", reflect.StructTag(tag).Get("etsjsapi"))
	}
}

func stringEnum(raw string) (string, error) {
	var values []string
	if err := json.Unmarshal([]byte(raw), &values); err != nil {
		return "", fmt.Errorf("invalid schema enum %q: %w", raw, err)
	}
	if len(values) == 0 {
		return "", fmt.Errorf("schema enum must not be empty")
	}
	quoted := make([]string, len(values))
	for index, value := range values {
		quoted[index] = strconv.Quote(value)
	}
	return strings.Join(quoted, " | "), nil
}

func jsonField(fallback string, tag string) (name string, optional bool, include bool, err error) {
	value := reflect.StructTag(tag).Get("json")
	if value == "" {
		return fallback, false, true, nil
	}
	parts := strings.Split(value, ",")
	if parts[0] == "-" {
		return "", false, false, nil
	}
	name = parts[0]
	if name == "" {
		name = fallback
	}
	for _, option := range parts[1:] {
		switch option {
		case "omitempty", "omitzero":
			optional = true
		case "":
		default:
			return "", false, false, fmt.Errorf("unsupported JSON tag option %q on protocol field %s", option, fallback)
		}
	}
	return name, optional, true, nil
}

func quoteProperty(value string) string {
	for index, char := range value {
		if !(char == '_' || char == '$' || char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' || index > 0 && char >= '0' && char <= '9') {
			return strconv.Quote(value)
		}
	}
	return value
}

func isProtocolTypePackage(path string) bool {
	return path == protocolPackage || path == effectCorePackage
}

func validateBasic(value *types.Basic) error {
	switch value.Kind() {
	case types.String, types.Bool,
		types.Int, types.Int8, types.Int16, types.Int32,
		types.Uint, types.Uint8, types.Uint16, types.Uint32,
		types.Float32, types.Float64:
		return nil
	default:
		return fmt.Errorf("unsupported protocol basic type %s", value.String())
	}
}
