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
	diagram := "flowchart TB\n  A --> B"
	baseURL := "https://mermaid.live/edit#"

	result := EncodeMermaidURL(baseURL, diagram)

	// Must start with base URL + "pako:"
	prefix := baseURL + "pako:"
	if !strings.HasPrefix(result, prefix) {
		t.Fatalf("expected prefix %q, got %q", prefix, result)
	}

	// Decode and verify roundtrip
	encoded := strings.TrimPrefix(result, prefix)
	compressed, err := base64.RawURLEncoding.DecodeString(encoded)
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

	if payload.Code != diagram {
		t.Errorf("roundtrip mismatch: got %q, want %q", payload.Code, diagram)
	}
}

func TestEncodeMermaidURL_CustomBase(t *testing.T) {
	t.Parallel()
	diagram := "flowchart LR\n  X --> Y"
	baseURL := "https://www.mermaidchart.com/play#"

	result := EncodeMermaidURL(baseURL, diagram)

	prefix := baseURL + "pako:"
	if !strings.HasPrefix(result, prefix) {
		t.Fatalf("expected prefix %q, got %q", prefix, result)
	}

	// Verify roundtrip with custom base
	encoded := strings.TrimPrefix(result, prefix)
	compressed, err := base64.RawURLEncoding.DecodeString(encoded)
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

	if payload.Code != diagram {
		t.Errorf("roundtrip mismatch: got %q, want %q", payload.Code, diagram)
	}
}

func TestEncodeMermaidURL_Deterministic(t *testing.T) {
	t.Parallel()
	diagram := "flowchart TB\n  A --> B --> C"
	baseURL := "https://mermaid.live/edit#"

	result1 := EncodeMermaidURL(baseURL, diagram)
	result2 := EncodeMermaidURL(baseURL, diagram)

	if result1 != result2 {
		t.Errorf("not deterministic: %q != %q", result1, result2)
	}
}
