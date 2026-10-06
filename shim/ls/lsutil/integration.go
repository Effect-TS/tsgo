package lsutil

import "github.com/microsoft/TypeScript/tsc/internal/ls/lsutil"

func NewInlayHintUserPreferences(preferences lsutil.InlayHintsPreferences) *lsutil.UserPreferences {
	return &lsutil.UserPreferences{InlayHintsPreferences: preferences}
}
