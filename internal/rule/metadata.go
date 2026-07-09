package rule

import "github.com/effect-ts/tsgo/etscore"

type MetadataGroup struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
}

type MetadataPreset struct {
	Name               string                      `json:"name"`
	Description        string                      `json:"description"`
	DiagnosticSeverity map[string]etscore.Severity `json:"diagnosticSeverity"`
}
