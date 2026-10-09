package lsutil

import "github.com/microsoft/typescript-go/shim/ls/lsutil"

func NewInlayHintUserPreferences(preferences lsutil.InlayHintsPreferences) *lsutil.UserPreferences {
	return &lsutil.UserPreferences{InlayHints: preferences}
}
