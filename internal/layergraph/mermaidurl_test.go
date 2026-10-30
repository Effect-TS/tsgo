package layergraph

import (
	"bytes"
	"compress/zlib"
	"encoding/base64"
	"encoding/json"
	"io"
	"strings"
	"testing"
)

func TestEncodeMermaidURL(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		baseURL string
		diagram string
	}{
		{"https://mermaid.live/edit#", "flowchart TB\n  A --> B"},
		{"https://www.mermaidchart.com/play#", "flowchart LR\n  X --> Y"},
	} {
		t.Run(tt.baseURL, func(t *testing.T) {
			t.Parallel()
			result := EncodeMermaidURL(tt.baseURL, tt.diagram)

			prefix := tt.baseURL + "pako:"
			if !strings.HasPrefix(result, prefix) {
				t.Fatalf("expected prefix %q, got %q", prefix, result)
			}

			compressed, err := base64.RawURLEncoding.DecodeString(strings.TrimPrefix(result, prefix))
			if err != nil {
				t.Fatalf("base64url decode failed: %v", err)
			}
			r, err := zlib.NewReader(bytes.NewReader(compressed))
			if err != nil {
				t.Fatalf("zlib reader failed: %v", err)
			}
			defer r.Close()
			decompressed, err := io.ReadAll(r)
			if err != nil {
				t.Fatalf("zlib decompress failed: %v", err)
			}
			var payload struct {
				Code string `json:"code"`
			}
			if err := json.Unmarshal(decompressed, &payload); err != nil {
				t.Fatalf("json unmarshal failed: %v", err)
			}
			if payload.Code != tt.diagram {
				t.Errorf("roundtrip mismatch: got %q, want %q", payload.Code, tt.diagram)
			}
		})
	}
}
