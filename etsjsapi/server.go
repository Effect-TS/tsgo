// Package etsjsapi implements the persistent Effect diagnostics bridge for JavaScript consumers.
package etsjsapi

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"sort"
	"strconv"
	"strings"
	"unicode/utf16"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/fixrunner"
	"github.com/effect-ts/tsgo/internal/pluginoptions"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/rulerunner"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	tsapi "github.com/microsoft/typescript-go/shim/api"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/collections"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/project"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/microsoft/typescript-go/shim/vfs/osvfs"
)

type request struct {
	Version int             `json:"version"`
	ID      int             `json:"id"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params"`
}

type response struct {
	Version int        `json:"version"`
	ID      int        `json:"id"`
	Result  any        `json:"result,omitempty"`
	Error   *wireError `json:"error,omitempty"`
}

type wireError struct {
	Message string `json:"message"`
}

type server struct {
	ctx            context.Context
	session        *project.Session
	baseSnapshot   *project.Snapshot
	openedFiles    map[string]struct{}
	openedProjects map[string]struct{}
}

// Run serves synchronous Effect JavaScript API requests until stdin closes.
func Run(ctx context.Context, args []string, stdin io.ReadCloser, stdout io.WriteCloser, stderr io.Writer) int {
	flags := flag.NewFlagSet("effect-js-api", flag.ContinueOnError)
	flags.SetOutput(stderr)
	cwd := flags.String("cwd", "", "current working directory")
	pipePath := flags.String("pipe", "", "use a named pipe or Unix domain socket instead of stdio")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if *cwd == "" {
		fmt.Fprintln(stderr, "--effect-js-api requires --cwd")
		return 2
	}

	fs := bundled.WrapFS(osvfs.FS())
	s := &server{
		ctx: ctx,
		session: project.NewSession(&project.SessionInit{
			BackgroundCtx: ctx,
			FS:            fs,
			Options: &project.SessionOptions{
				CurrentDirectory:   *cwd,
				DefaultLibraryPath: bundled.LibPath(),
				PositionEncoding:   lsproto.PositionEncodingKindUTF16,
			},
		}),
		openedFiles:    make(map[string]struct{}),
		openedProjects: make(map[string]struct{}),
	}
	defer s.close()

	var transport tsapi.Transport
	if *pipePath == "" {
		transport = tsapi.NewStdioTransport(stdin, stdout)
	} else {
		pipeTransport, err := tsapi.NewPipeTransport(*pipePath)
		if err != nil {
			fmt.Fprintln(stderr, err)
			return 1
		}
		transport = pipeTransport
	}
	defer transport.Close()
	connection, err := transport.Accept()
	if err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}

	reader := bufio.NewReader(connection)
	writer := bufio.NewWriter(connection)
	for {
		payload, err := readFrame(reader)
		if errors.Is(err, io.EOF) {
			return 0
		}
		if err != nil {
			fmt.Fprintln(stderr, err)
			return 1
		}

		var req request
		if err := json.Unmarshal(payload, &req); err != nil {
			fmt.Fprintln(stderr, err)
			return 1
		}
		resp := s.handle(req)
		if err := writeFrame(writer, resp); err != nil {
			fmt.Fprintln(stderr, err)
			return 1
		}
	}
}

func (s *server) handle(req request) response {
	resp := response{Version: ProtocolVersion, ID: req.ID}
	if req.Version != ProtocolVersion {
		resp.Error = &wireError{Message: fmt.Sprintf("unsupported protocol version %d", req.Version)}
		return resp
	}
	if req.Method != runEffectDiagnosticsMethod.Name {
		resp.Error = &wireError{Message: fmt.Sprintf("unsupported method %q", req.Method)}
		return resp
	}

	var params RunEffectDiagnosticsParams
	if err := json.Unmarshal(req.Params, &params); err != nil {
		resp.Error = &wireError{Message: err.Error()}
		return resp
	}
	result, err := s.runEffectDiagnostics(params)
	if err != nil {
		resp.Error = &wireError{Message: err.Error()}
		return resp
	}
	resp.Result = result
	return resp
}

func (s *server) runEffectDiagnostics(params RunEffectDiagnosticsParams) (*RunEffectDiagnosticsResult, error) {
	if params.TargetFilePath == "" {
		return nil, errors.New("runEffectDiagnostics request is missing targetFilePath")
	}

	apiRequest := &project.APISnapshotRequest{}
	openProject := ""
	if params.ProjectFilePath != "" {
		if _, opened := s.openedProjects[params.ProjectFilePath]; !opened {
			apiRequest.OpenProjects = &collections.Set[string]{}
			apiRequest.OpenProjects.Add(params.ProjectFilePath)
			openProject = params.ProjectFilePath
		}
	}
	uri := lsconv.FileNameToDocumentURI(params.TargetFilePath)
	openFile := false
	if _, opened := s.openedFiles[params.TargetFilePath]; !opened {
		apiRequest.OpenFiles = &collections.Set[lsproto.DocumentUri]{}
		apiRequest.OpenFiles.Add(uri)
		openFile = true
	}
	next, err := s.session.APIUpdate(s.ctx, project.FileChangeSummary{}, apiRequest)
	if err != nil {
		if next != nil {
			next.Deref(s.session)
		}
		return nil, err
	}
	if s.baseSnapshot != nil {
		s.baseSnapshot.Deref(s.session)
	}
	s.baseSnapshot = next
	if openProject != "" {
		s.openedProjects[openProject] = struct{}{}
	}
	if openFile {
		s.openedFiles[params.TargetFilePath] = struct{}{}
	}

	snapshot := s.baseSnapshot
	if params.OverrideSourceText != nil {
		temporary, err := s.session.APIUpdateTemporary(s.ctx, s.baseSnapshot, uri, *params.OverrideSourceText)
		if err != nil {
			return nil, err
		}
		defer temporary.Deref(s.session)
		snapshot = temporary
	}

	configuredProject := snapshot.GetDefaultProject(uri)
	if configuredProject == nil || configuredProject.GetProgram() == nil {
		return nil, fmt.Errorf("no TypeScript project contains %s", params.TargetFilePath)
	}
	program := configuredProject.GetProgram()
	sourceFile := program.GetSourceFile(params.TargetFilePath)
	if sourceFile == nil {
		return nil, fmt.Errorf("TypeScript project did not load %s", params.TargetFilePath)
	}
	effectOptions, optionsSource, err := resolveEffectOptions(params.OverrideEffectOptions, program.Options().Effect)
	if err != nil {
		return nil, err
	}
	var onlyRules []string
	if params.OnlyRules != nil {
		onlyRules = *params.OnlyRules
		effectOptions = normalizeSeverities(effectOptions, onlyRules)
	}

	checker, done := program.GetTypeChecker(core.WithCheckerLifetime(s.ctx, core.CheckerLifetimeAPI))
	defer done()
	diagnostics, err := rulerunner.Run(s.ctx, program, checker, sourceFile, effectOptions, onlyRules)
	if err != nil {
		return nil, err
	}

	result := &RunEffectDiagnosticsResult{
		Diagnostics:   make([]Diagnostic, 0, len(diagnostics)),
		OptionsSource: optionsSource,
	}
	requestedRules := make(map[string]struct{}, len(onlyRules))
	for _, ruleName := range onlyRules {
		requestedRules[ruleName] = struct{}{}
	}
	var languageService *ls.LanguageService
	var resolvedOptions *etscore.ResolvedEffectPluginOptions
	var tp *typeparser.TypeParser
	if params.IncludeFixes {
		languageService = ls.NewLanguageService(configuredProject.ID(), program, snapshot, params.TargetFilePath)
		resolvedOptions = pluginoptions.ResolveEffectPluginOptionsForSourceFile(
			effectOptions,
			sourceFile.FileName(),
			program.Options().ConfigFilePath,
			program.UseCaseSensitiveFileNames(),
		)
		tp = typeparser.NewTypeParser(program, checker)
	}
	for _, item := range diagnostics {
		formatted := formatDiagnostic(item)
		if params.OnlyRules != nil {
			if _, requested := requestedRules[formatted.RuleName]; !requested {
				continue
			}
		}
		if params.IncludeFixes {
			formatted.Actions = formatCodeActions(s.ctx, item, program, sourceFile, languageService, resolvedOptions, checker, tp)
		}
		result.Diagnostics = append(result.Diagnostics, formatted)
	}
	return result, nil
}

func formatCodeActions(
	ctx context.Context,
	diagnostic *ast.Diagnostic,
	program *compiler.Program,
	sourceFile *ast.SourceFile,
	languageService *ls.LanguageService,
	options *etscore.ResolvedEffectPluginOptions,
	checker *checker.Checker,
	tp *typeparser.TypeParser,
) []CodeAction {
	fixCtx := &ls.CodeFixContext{
		SourceFile: sourceFile,
		Span:       core.NewTextRange(diagnostic.Pos(), diagnostic.End()),
		ErrorCode:  diagnostic.Code(),
		Program:    program,
		LS:         languageService,
	}
	converters := ls.LanguageService_converters(languageService)
	utf16Length := len(utf16.Encode([]rune(sourceFile.Text())))
	seen := make(map[string]struct{})
	var actions []CodeAction
	for _, action := range fixrunner.CollectActions(ctx, fixCtx, options, checker, tp, false) {
		edits := make([]TextEdit, 0, len(action.Changes))
		valid := true
		for _, change := range action.Changes {
			startByte := int(converters.LineAndCharacterToPosition(sourceFile, change.Range.Start))
			endByte := int(converters.LineAndCharacterToPosition(sourceFile, change.Range.End))
			if !positionMatches(sourceFile, change.Range.Start, startByte) || !positionMatches(sourceFile, change.Range.End, endByte) {
				valid = false
				break
			}
			start := sourceFile.GetPositionMap().UTF8ToUTF16(startByte)
			end := sourceFile.GetPositionMap().UTF8ToUTF16(endByte)
			if start < 0 || end < start || end > utf16Length {
				valid = false
				break
			}
			edits = append(edits, TextEdit{Start: start, End: end, NewText: change.NewText})
		}
		if !valid || len(edits) == 0 {
			continue
		}
		sort.SliceStable(edits, func(i int, j int) bool {
			if edits[i].Start != edits[j].Start {
				return edits[i].Start < edits[j].Start
			}
			return edits[i].End < edits[j].End
		})
		for index := 1; index < len(edits); index++ {
			if edits[index].Start < edits[index-1].End {
				valid = false
				break
			}
		}
		if !valid {
			continue
		}
		formatted := CodeAction{Title: action.Description, Edits: edits}
		keyBytes, err := json.Marshal(formatted)
		if err != nil {
			continue
		}
		key := string(keyBytes)
		if _, exists := seen[key]; exists {
			continue
		}
		seen[key] = struct{}{}
		actions = append(actions, formatted)
	}
	return actions
}

func positionMatches(sourceFile *ast.SourceFile, position lsproto.Position, bytePosition int) bool {
	line, character := scanner.GetECMALineAndUTF16CharacterOfPosition(sourceFile, bytePosition)
	return uint32(line) == position.Line && uint32(character) == position.Character
}

func resolveEffectOptions(options *etscore.EffectPluginOptions, fallback *etscore.EffectPluginOptions) (*etscore.EffectPluginOptions, OptionsSource, error) {
	if options != nil {
		return options, OptionsSourceSettings, nil
	}
	if fallback == nil {
		return nil, "", errors.New("TypeScript project does not enable @effect/language-service diagnostics and no effect-tsgo setting was provided")
	}
	cloned, err := cloneEffectOptions(fallback)
	if err != nil {
		return nil, "", err
	}
	return cloned, OptionsSourceTSConfig, nil
}

func cloneEffectOptions(options *etscore.EffectPluginOptions) (*etscore.EffectPluginOptions, error) {
	payload, err := json.Marshal(options)
	if err != nil {
		return nil, err
	}
	var cloned etscore.EffectPluginOptions
	if err := json.Unmarshal(payload, &cloned); err != nil {
		return nil, err
	}
	return &cloned, nil
}

func normalizeSeverities(options *etscore.EffectPluginOptions, requestedRules []string) *etscore.EffectPluginOptions {
	options.Diagnostics = true
	options.DiagnosticSeverity = make(map[string]etscore.Severity, len(rules.All))
	for _, registeredRule := range rules.All {
		options.DiagnosticSeverity[registeredRule.Name] = etscore.SeverityOff
	}
	for _, ruleName := range requestedRules {
		options.DiagnosticSeverity[ruleName] = etscore.SeverityError
	}
	for index := range options.Overrides {
		options.Overrides[index].Options.DiagnosticSeverity = nil
	}
	return options
}

func (s *server) close() {
	if s.baseSnapshot != nil {
		s.baseSnapshot.Deref(s.session)
	}
	s.session.Close()
}

func formatDiagnostic(item *ast.Diagnostic) Diagnostic {
	file := item.File()
	start := file.GetPositionMap().UTF8ToUTF16(item.Pos())
	end := file.GetPositionMap().UTF8ToUTF16(item.End())
	ruleName := rule.CodeToRuleName(rules.All, item.Code())
	message := flattenMessage(item, 0)
	message = strings.TrimSuffix(message, " effect("+ruleName+")")
	return Diagnostic{
		File:     file.FileName(),
		Start:    start,
		End:      end,
		Code:     item.Code(),
		RuleName: ruleName,
		Message:  message,
	}
}

func flattenMessage(item *ast.Diagnostic, level int) string {
	var output strings.Builder
	output.WriteString(item.String())
	for _, child := range item.MessageChain() {
		output.WriteByte('\n')
		output.WriteString(strings.Repeat("  ", level+1))
		output.WriteString(flattenMessage(child, level+1))
	}
	return output.String()
}

func readFrame(reader *bufio.Reader) ([]byte, error) {
	contentLength := -1
	for {
		line, err := reader.ReadString('\n')
		if err != nil {
			return nil, err
		}
		line = strings.TrimSuffix(strings.TrimSuffix(line, "\n"), "\r")
		if line == "" {
			break
		}
		name, value, ok := strings.Cut(line, ":")
		if !ok || !strings.EqualFold(strings.TrimSpace(name), "Content-Length") {
			continue
		}
		contentLength, err = strconv.Atoi(strings.TrimSpace(value))
		if err != nil || contentLength < 0 {
			return nil, fmt.Errorf("invalid Content-Length %q", value)
		}
	}
	if contentLength < 0 {
		return nil, errors.New("missing Content-Length header")
	}
	payload := make([]byte, contentLength)
	if _, err := io.ReadFull(reader, payload); err != nil {
		return nil, err
	}
	return payload, nil
}

func writeFrame(writer *bufio.Writer, value any) error {
	payload, err := json.Marshal(value)
	if err != nil {
		return err
	}
	if _, err := fmt.Fprintf(writer, "Content-Length: %d\r\n\r\n", len(payload)); err != nil {
		return err
	}
	if _, err := writer.Write(payload); err != nil {
		return err
	}
	return writer.Flush()
}
