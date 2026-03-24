package rule

import "github.com/effect-ts/effect-typescript-go/etscore"

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
