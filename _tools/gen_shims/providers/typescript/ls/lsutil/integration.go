package lsutil

import "github.com/microsoft/typescript-go/internal/ls/lsutil"

func NewInlayHintUserPreferences(preferences lsutil.InlayHintsPreferences) *lsutil.UserPreferences {
	return &lsutil.UserPreferences{InlayHintsPreferences: preferences}
}
