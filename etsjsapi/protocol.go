package etsjsapi

import (
	"encoding/json"
	"errors"
	"fmt"

	"github.com/effect-ts/tsgo/etscore"
)

//go:generate go run ../_tools/gen_etsjsapi

// ProtocolVersion is the current Effect JavaScript API wire protocol version.
const ProtocolVersion = 3

type method[Params, Result any] struct {
	Name string
}

var runEffectDiagnosticsMethod = method[RunEffectDiagnosticsParams, RunEffectDiagnosticsResult]{Name: "runEffectDiagnostics"}

// OptionsSource identifies where the Effect options for a request came from.
type OptionsSource string

const (
	OptionsSourceSettings OptionsSource = "settings"
	OptionsSourceTSConfig OptionsSource = "tsconfig"
)

// RunEffectDiagnosticsParams contains the inputs for a runEffectDiagnostics request.
type RunEffectDiagnosticsParams struct {
	TargetFilePath        string                       `json:"targetFilePath"`
	OverrideSourceText    *string                      `json:"overrideSourceText,omitempty"`
	ProjectFilePath       string                       `json:"projectFilePath,omitempty"`
	OnlyRules             *[]string                    `json:"onlyRules,omitempty" etsjsapi:"nullable"`
	OverrideEffectOptions *etscore.EffectPluginOptions `json:"overrideEffectOptions,omitempty"`
	IncludeFixes          bool                         `json:"includeFixes,omitempty"`
}

// UnmarshalJSON preserves the defaults and normalization used for Effect option overrides.
func (p *RunEffectDiagnosticsParams) UnmarshalJSON(data []byte) error {
	type params RunEffectDiagnosticsParams
	decoded := struct {
		*params
		OverrideEffectOptions json.RawMessage `json:"overrideEffectOptions"`
	}{params: (*params)(p)}
	*p = RunEffectDiagnosticsParams{}
	if err := json.Unmarshal(data, &decoded); err != nil {
		return err
	}
	if len(decoded.OverrideEffectOptions) == 0 {
		return nil
	}

	var config any
	if err := json.Unmarshal(decoded.OverrideEffectOptions, &config); err != nil {
		return fmt.Errorf("invalid effect-tsgo setting: %w", err)
	}
	settings, ok := config.(map[string]any)
	if !ok {
		return errors.New("context.settings[\"effect-tsgo\"] must be an object")
	}
	settings["name"] = etscore.EffectPluginName
	p.OverrideEffectOptions = etscore.ParseFromPlugins([]any{settings})
	if p.OverrideEffectOptions == nil {
		return errors.New("unable to parse context.settings[\"effect-tsgo\"]")
	}
	return nil
}

// RunEffectDiagnosticsResult contains diagnostics and their optional code actions.
type RunEffectDiagnosticsResult struct {
	Diagnostics   []Diagnostic  `json:"diagnostics"`
	OptionsSource OptionsSource `json:"optionsSource"`
}

// Diagnostic is an Effect diagnostic using UTF-16 offsets.
type Diagnostic struct {
	File     string       `json:"file"`
	Start    int          `json:"start"`
	End      int          `json:"end"`
	Code     int32        `json:"code"`
	RuleName string       `json:"ruleName"`
	Message  string       `json:"message"`
	Actions  []CodeAction `json:"actions,omitempty"`
}

// CodeAction is one suggestion composed of one or more text edits.
type CodeAction struct {
	Title string     `json:"title"`
	Edits []TextEdit `json:"edits"`
}

// TextEdit replaces a UTF-16 range with new text.
type TextEdit struct {
	Start   int    `json:"start"`
	End     int    `json:"end"`
	NewText string `json:"newText"`
}
