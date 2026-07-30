// Package etsoxlint implements the persistent Effect diagnostics bridge for Oxlint.
package etsoxlint

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"strconv"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/rulerunner"
	"github.com/effect-ts/tsgo/internal/rules"
	tsapi "github.com/microsoft/typescript-go/shim/api"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/collections"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/project"
	"github.com/microsoft/typescript-go/shim/vfs/osvfs"
)

const protocolVersion = 1

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

type lintParams struct {
	File          string          `json:"file"`
	Text          string          `json:"text"`
	Project       string          `json:"project,omitempty"`
	Rules         []string        `json:"rules"`
	EffectOptions json.RawMessage `json:"effectOptions,omitempty"`
}

type lintResult struct {
	Diagnostics   []diagnostic `json:"diagnostics"`
	OptionsSource string       `json:"optionsSource"`
}

type diagnostic struct {
	File     string `json:"file"`
	Start    int    `json:"start"`
	End      int    `json:"end"`
	Code     int32  `json:"code"`
	RuleName string `json:"ruleName"`
	Message  string `json:"message"`
}

type server struct {
	ctx            context.Context
	session        *project.Session
	baseSnapshot   *project.Snapshot
	openedFiles    map[string]struct{}
	openedProjects map[string]struct{}
}

// Run serves synchronous Effect Oxlint requests until stdin closes.
func Run(ctx context.Context, args []string, stdin io.ReadCloser, stdout io.WriteCloser, stderr io.Writer) int {
	flags := flag.NewFlagSet("effect-oxlint", flag.ContinueOnError)
	flags.SetOutput(stderr)
	cwd := flags.String("cwd", "", "current working directory")
	pipePath := flags.String("pipe", "", "use a named pipe or Unix domain socket instead of stdio")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if *cwd == "" {
		fmt.Fprintln(stderr, "--effect-oxlint requires --cwd")
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
				PositionEncoding:   lsproto.PositionEncodingKindUTF8,
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
	resp := response{Version: protocolVersion, ID: req.ID}
	if req.Version != protocolVersion {
		resp.Error = &wireError{Message: fmt.Sprintf("unsupported protocol version %d", req.Version)}
		return resp
	}
	if req.Method != "lint" {
		resp.Error = &wireError{Message: fmt.Sprintf("unsupported method %q", req.Method)}
		return resp
	}

	var params lintParams
	if err := json.Unmarshal(req.Params, &params); err != nil {
		resp.Error = &wireError{Message: err.Error()}
		return resp
	}
	result, err := s.lint(params)
	if err != nil {
		resp.Error = &wireError{Message: err.Error()}
		return resp
	}
	resp.Result = result
	return resp
}

func (s *server) lint(params lintParams) (*lintResult, error) {
	if params.File == "" {
		return nil, errors.New("lint request is missing file")
	}

	apiRequest := &project.APISnapshotRequest{}
	updateBase := s.baseSnapshot == nil
	openProject := ""
	if params.Project != "" {
		if _, opened := s.openedProjects[params.Project]; !opened {
			apiRequest.OpenProjects = &collections.Set[string]{}
			apiRequest.OpenProjects.Add(params.Project)
			openProject = params.Project
			updateBase = true
		}
	}
	uri := lsconv.FileNameToDocumentURI(params.File)
	openFile := false
	if _, opened := s.openedFiles[params.File]; !opened {
		apiRequest.OpenFiles = &collections.Set[lsproto.DocumentUri]{}
		apiRequest.OpenFiles.Add(uri)
		openFile = true
		updateBase = true
	}
	if updateBase {
		next, err := s.session.APIUpdate(s.ctx, project.FileChangeSummary{}, apiRequest)
		if err != nil {
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
			s.openedFiles[params.File] = struct{}{}
		}
	}

	temporary, err := s.session.APIUpdateTemporary(s.ctx, s.baseSnapshot, uri, params.Text)
	if err != nil {
		return nil, err
	}
	defer temporary.Deref(s.session)

	configuredProject := temporary.GetDefaultProject(uri)
	if configuredProject == nil || configuredProject.GetProgram() == nil {
		return nil, fmt.Errorf("no TypeScript project contains %s", params.File)
	}
	program := configuredProject.GetProgram()
	sourceFile := program.GetSourceFile(params.File)
	if sourceFile == nil {
		return nil, fmt.Errorf("TypeScript project did not load %s", params.File)
	}
	effectOptions, optionsSource, err := resolveEffectOptions(params.EffectOptions, program.Options().Effect)
	if err != nil {
		return nil, err
	}
	effectOptions = normalizeSeverities(effectOptions, params.Rules)

	checker, done := program.GetTypeChecker(core.WithCheckerLifetime(s.ctx, core.CheckerLifetimeAPI))
	defer done()
	diagnostics, err := rulerunner.Run(s.ctx, program, checker, sourceFile, effectOptions, params.Rules)
	if err != nil {
		return nil, err
	}

	result := &lintResult{
		Diagnostics:   make([]diagnostic, 0, len(diagnostics)),
		OptionsSource: optionsSource,
	}
	for _, item := range diagnostics {
		result.Diagnostics = append(result.Diagnostics, formatDiagnostic(item))
	}
	return result, nil
}

func resolveEffectOptions(raw json.RawMessage, fallback *etscore.EffectPluginOptions) (*etscore.EffectPluginOptions, string, error) {
	if len(raw) != 0 {
		var decoded any
		if err := json.Unmarshal(raw, &decoded); err != nil {
			return nil, "", fmt.Errorf("invalid effect-tsgo setting: %w", err)
		}
		config, ok := decoded.(map[string]any)
		if !ok {
			return nil, "", errors.New("context.settings[\"effect-tsgo\"] must be an object")
		}
		config["name"] = etscore.EffectPluginName
		parsed := etscore.ParseFromPlugins([]any{config})
		if parsed == nil {
			return nil, "", errors.New("unable to parse context.settings[\"effect-tsgo\"]")
		}
		return parsed, "settings", nil
	}
	if fallback == nil {
		return nil, "", errors.New("TypeScript project does not enable @effect/language-service diagnostics and no effect-tsgo setting was provided")
	}
	cloned, err := cloneEffectOptions(fallback)
	if err != nil {
		return nil, "", err
	}
	return cloned, "tsconfig", nil
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

func formatDiagnostic(item *ast.Diagnostic) diagnostic {
	file := item.File()
	start := file.GetPositionMap().UTF8ToUTF16(item.Pos())
	end := file.GetPositionMap().UTF8ToUTF16(item.End())
	ruleName := rule.CodeToRuleName(rules.All, item.Code())
	message := flattenMessage(item, 0)
	message = strings.TrimSuffix(message, " effect("+ruleName+")")
	return diagnostic{
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
