// Package etsdiagnostics implements the native Effect diagnostics CLI mode.
package etsdiagnostics

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"maps"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/bundled"
	"github.com/microsoft/typescript-go/shim/collections"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/project"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/microsoft/typescript-go/shim/tspath"
	"github.com/microsoft/typescript-go/shim/vfs/osvfs"
)

const noFilesMessage = "No files to check. Please provide an existing .ts file or a project tsconfig.json"

type request struct {
	CWD       string  `json:"cwd"`
	File      string  `json:"file,omitempty"`
	Project   string  `json:"project,omitempty"`
	Format    string  `json:"format"`
	Strict    bool    `json:"strict"`
	Severity  string  `json:"severity,omitempty"`
	Progress  bool    `json:"progress"`
	LSPConfig *string `json:"lspconfig,omitempty"`
}

type severity string

const (
	severityError   severity = "error"
	severityWarning severity = "warning"
	severityMessage severity = "message"
)

type formattedDiagnostic struct {
	File      string   `json:"file"`
	Start     int      `json:"start"`
	Length    int      `json:"length"`
	Line      int      `json:"line"`
	Column    int      `json:"column"`
	EndLine   int      `json:"endLine"`
	EndColumn int      `json:"endColumn"`
	Severity  severity `json:"severity"`
	Code      int32    `json:"code"`
	Name      string   `json:"name"`
	Message   string   `json:"message"`
	source    string
}

type summary struct {
	FilesChecked int `json:"filesChecked"`
	TotalFiles   int `json:"totalFiles"`
	Errors       int `json:"errors"`
	Warnings     int `json:"warnings"`
	Messages     int `json:"messages"`
}

type jsonOutput struct {
	Diagnostics []formattedDiagnostic `json:"diagnostics"`
	Summary     summary               `json:"summary"`
}

// Run decodes and executes one JSON diagnostics request.
func Run(ctx context.Context, args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) != 1 {
		fmt.Fprintln(stderr, "expected one JSON request after --effect-cli-diagnostics")
		return 2
	}

	var req request
	if err := json.Unmarshal([]byte(args[0]), &req); err != nil {
		fmt.Fprintf(stderr, "invalid diagnostics request: %v\n", err)
		return 2
	}
	if req.CWD == "" {
		fmt.Fprintln(stderr, "diagnostics request is missing cwd")
		return 2
	}
	if req.Format == "" {
		req.Format = "pretty"
	}
	if req.Format != "json" && req.Format != "pretty" && req.Format != "text" && req.Format != "github-actions" {
		fmt.Fprintf(stderr, "unsupported diagnostics format %q\n", req.Format)
		return 2
	}

	override, overrideProvided, err := parseLSPConfig(req.LSPConfig)
	if err != nil {
		fmt.Fprintf(stderr, "Invalid JSON lsp config: %s\n", *req.LSPConfig)
		return 1
	}

	diagnostics, resultSummary, err := collect(ctx, req, override, overrideProvided, stderr)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	if err := writeOutput(stdout, req.Format, diagnostics, resultSummary); err != nil {
		fmt.Fprintf(stderr, "unable to write diagnostics: %v\n", err)
		return 2
	}
	if resultSummary.Errors > 0 || (req.Strict && resultSummary.Warnings > 0) {
		return 1
	}
	return 0
}

func parseLSPConfig(value *string) (*etscore.EffectPluginOptions, bool, error) {
	if value == nil {
		return nil, false, nil
	}
	var decoded any
	if err := json.Unmarshal([]byte(*value), &decoded); err != nil {
		return nil, true, err
	}
	config := make(map[string]any)
	if object, ok := decoded.(map[string]any); ok {
		maps.Copy(config, object)
	}
	config["name"] = etscore.EffectPluginName
	return etscore.ParseFromPlugins([]any{config}), true, nil
}

func collect(ctx context.Context, req request, override *etscore.EffectPluginOptions, overrideProvided bool, stderr io.Writer) ([]formattedDiagnostic, summary, error) {
	fs := bundled.WrapFS(osvfs.FS())
	session := project.NewSession(&project.SessionInit{
		BackgroundCtx: ctx,
		FS:            fs,
		Options: &project.SessionOptions{
			CurrentDirectory:   req.CWD,
			DefaultLibraryPath: bundled.LibPath(),
			PositionEncoding:   lsproto.PositionEncodingKindUTF8,
		},
	})
	defer session.Close()

	targets := make([]string, 0)
	seenTargets := make(map[tspath.Path]struct{})
	addTarget := func(fileName string) {
		path := tspath.ToPath(fileName, req.CWD, fs.UseCaseSensitiveFileNames())
		if _, seen := seenTargets[path]; seen {
			return
		}
		seenTargets[path] = struct{}{}
		targets = append(targets, fileName)
	}

	if req.Project != "" {
		projectName := tspath.GetNormalizedAbsolutePath(req.Project, req.CWD)
		if fs.DirectoryExists(projectName) {
			projectName = tspath.CombinePaths(projectName, "tsconfig.json")
		}
		openProjects := &collections.Set[string]{}
		openProjects.Add(projectName)
		if err := updateSession(ctx, session, &project.APISnapshotRequest{OpenProjects: openProjects}); err != nil {
			return nil, summary{}, err
		}

		session.WithSnapshotLoadingProjectTree(ctx, nil, func(snapshot *project.Snapshot) {
			for _, configuredProject := range snapshot.ProjectCollection.ConfiguredProjects() {
				if configuredProject.CommandLine == nil {
					continue
				}
				for _, fileName := range configuredProject.CommandLine.FileNames() {
					addTarget(fileName)
				}
			}
		})
	}

	if req.File != "" {
		fileName := tspath.GetNormalizedAbsolutePath(req.File, req.CWD)
		uri := lsconv.FileNameToDocumentURI(fileName)
		openFiles := &collections.Set[lsproto.DocumentUri]{}
		openFiles.Add(uri)
		if err := updateSession(ctx, session, &project.APISnapshotRequest{OpenFiles: openFiles}); err != nil {
			return nil, summary{}, err
		}
		addTarget(fileName)
	}

	resultSummary := summary{TotalFiles: len(targets)}
	if len(targets) == 0 {
		return nil, resultSummary, fmt.Errorf("%s", noFilesMessage)
	}

	severityFilter := parseSeverityFilter(req.Severity)
	results := make([]formattedDiagnostic, 0)
	ctx = core.WithCheckerLifetime(ctx, core.CheckerLifetimeDiagnostics)
	if req.Progress {
		fmt.Fprintf(stderr, "Starting diagnostics for %d files...\n", len(targets))
	}

	var collectErr error
	session.WithSnapshotLoadingProjectTree(ctx, nil, func(snapshot *project.Snapshot) {
		for index, fileName := range targets {
			if req.Progress {
				fmt.Fprintf(stderr, "[%d/%d] %60s\r", index+1, len(targets), truncateLeft(fileName, 60))
			}
			uri := lsconv.FileNameToDocumentURI(fileName)
			configuredProject := snapshot.GetDefaultProject(uri)
			if configuredProject == nil || configuredProject.GetProgram() == nil {
				continue
			}
			program := configuredProject.GetProgram()
			sourceFile := program.GetSourceFile(fileName)
			if sourceFile == nil {
				continue
			}
			if overrideProvided {
				program.Options().Effect = override
			}
			if program.Options().Effect == nil {
				continue
			}

			for _, diagnostic := range program.GetSemanticDiagnostics(ctx, sourceFile) {
				if !rule.IsEffectCode(diagnostic.Code()) {
					continue
				}
				formatted := formatDiagnostic(diagnostic)
				if severityFilter != nil {
					if _, included := severityFilter[formatted.Severity]; !included {
						continue
					}
				}
				results = append(results, formatted)
				switch formatted.Severity {
				case severityError:
					resultSummary.Errors++
				case severityWarning:
					resultSummary.Warnings++
				default:
					resultSummary.Messages++
				}
			}
			resultSummary.FilesChecked++
			if err := ctx.Err(); err != nil {
				collectErr = err
				return
			}
		}
	})
	if req.Progress {
		fmt.Fprintln(stderr)
	}
	if collectErr != nil {
		return nil, resultSummary, collectErr
	}
	return results, resultSummary, nil
}

func updateSession(ctx context.Context, session *project.Session, request *project.APISnapshotRequest) error {
	snapshot, err := session.APIUpdate(ctx, project.FileChangeSummary{}, request)
	if snapshot != nil {
		snapshot.Deref(session)
	}
	return err
}

func parseSeverityFilter(value string) map[severity]struct{} {
	if value == "" {
		return nil
	}
	result := make(map[severity]struct{})
	for item := range strings.SplitSeq(value, ",") {
		level := severity(strings.ToLower(strings.TrimSpace(item)))
		if level == severityError || level == severityWarning || level == severityMessage {
			result[level] = struct{}{}
		}
	}
	if len(result) == 0 {
		return nil
	}
	return result
}

func formatDiagnostic(diagnostic *ast.Diagnostic) formattedDiagnostic {
	file := diagnostic.File()
	start := file.GetPositionMap().UTF8ToUTF16(diagnostic.Pos())
	end := file.GetPositionMap().UTF8ToUTF16(diagnostic.End())
	line, column := scanner.GetECMALineAndUTF16CharacterOfPosition(file, diagnostic.Pos())
	endLine, endColumn := scanner.GetECMALineAndUTF16CharacterOfPosition(file, diagnostic.End())
	name := rule.CodeToRuleName(rules.All, diagnostic.Code())
	if name == "" {
		name = fmt.Sprintf("effect(%d)", diagnostic.Code())
	}
	message := flattenMessage(diagnostic, 0)
	message = strings.TrimSuffix(message, " effect("+name+")")
	return formattedDiagnostic{
		File:      file.FileName(),
		Start:     start,
		Length:    end - start,
		Line:      line + 1,
		Column:    int(column) + 1,
		EndLine:   endLine + 1,
		EndColumn: int(endColumn) + 1,
		Severity:  categoryToSeverity(diagnostic.Category()),
		Code:      diagnostic.Code(),
		Name:      name,
		Message:   message,
		source:    file.Text(),
	}
}

func flattenMessage(diagnostic *ast.Diagnostic, level int) string {
	var output strings.Builder
	output.WriteString(diagnostic.String())
	for _, child := range diagnostic.MessageChain() {
		output.WriteByte('\n')
		output.WriteString(strings.Repeat("  ", level+1))
		output.WriteString(flattenMessage(child, level+1))
	}
	return output.String()
}

func categoryToSeverity(category tsdiag.Category) severity {
	switch category {
	case tsdiag.CategoryError:
		return severityError
	case tsdiag.CategoryWarning:
		return severityWarning
	default:
		return severityMessage
	}
}

func writeOutput(output io.Writer, format string, diagnostics []formattedDiagnostic, resultSummary summary) error {
	switch format {
	case "json":
		encoder := json.NewEncoder(output)
		encoder.SetIndent("", "  ")
		return encoder.Encode(jsonOutput{Diagnostics: diagnostics, Summary: resultSummary})
	case "github-actions":
		for _, diagnostic := range diagnostics {
			command := string(diagnostic.Severity)
			if diagnostic.Severity == severityMessage {
				command = "notice"
			}
			message := strings.NewReplacer("%", "%25", "\r", "%0D", "\n", "%0A").Replace(diagnostic.Message)
			fmt.Fprintf(output, "::%s file=%s,line=%d,col=%d,endLine=%d,endColumn=%d,title=%s::%s\n",
				command, diagnostic.File, diagnostic.Line, diagnostic.Column, diagnostic.EndLine, diagnostic.EndColumn, diagnostic.Name, message)
		}
	case "text":
		for _, diagnostic := range diagnostics {
			fmt.Fprintf(output, "%s(%d,%d): %s %s: %s\n",
				diagnostic.File, diagnostic.Line, diagnostic.Column, diagnostic.Severity, diagnosticLabel(diagnostic.Name), diagnostic.Message)
		}
	case "pretty":
		for _, diagnostic := range diagnostics {
			writePrettyDiagnostic(output, diagnostic)
		}
	}
	_, err := fmt.Fprintf(output, "Checked %d files out of %d files. \n%d errors, %d warnings and %d messages.\n",
		resultSummary.FilesChecked, resultSummary.TotalFiles, resultSummary.Errors, resultSummary.Warnings, resultSummary.Messages)
	return err
}

func writePrettyDiagnostic(output io.Writer, diagnostic formattedDiagnostic) {
	var color string
	switch diagnostic.Severity {
	case severityError:
		color = "\x1b[91m"
	case severityWarning:
		color = "\x1b[93m"
	default:
		color = "\x1b[96m"
	}
	reset := "\x1b[0m"
	fmt.Fprintf(output, "%s%s:%d:%d - %s %s:%s %s\n",
		color, diagnostic.File, diagnostic.Line, diagnostic.Column, diagnostic.Severity, diagnosticLabel(diagnostic.Name), reset, diagnostic.Message)
	lines := strings.Split(diagnostic.source, "\n")
	if diagnostic.Line > 0 && diagnostic.Line <= len(lines) {
		line := strings.TrimSuffix(lines[diagnostic.Line-1], "\r")
		fmt.Fprintf(output, "\n%d %s\n  %s%s%s\n\n", diagnostic.Line, line, color,
			strings.Repeat(" ", max(diagnostic.Column-1, 0))+strings.Repeat("~", max(diagnostic.EndColumn-diagnostic.Column, 1)), reset)
	}
}

func diagnosticLabel(name string) string {
	if strings.HasPrefix(name, "effect(") {
		return name
	}
	return "effect(" + name + ")"
}

func truncateLeft(value string, width int) string {
	if len(value) <= width {
		return value
	}
	return value[len(value)-width:]
}
