package layergraph

import (
	"bytes"
	"compress/zlib"
	"encoding/base64"
	"encoding/json"
)

// EncodeMermaidURL encodes a Mermaid diagram string into a pako-compatible URL fragment.
// It JSON-serializes {"code": diagram}, compresses with zlib at max compression,
// base64url-encodes (no padding), and returns baseURL + "pako:" + encoded.
func EncodeMermaidURL(baseURL string, diagram string) string {
	payload := struct {
		Code string `json:"code"`
	}{Code: diagram}
	jsonBytes, err := json.Marshal(payload)
	if err != nil {
		return baseURL
	}

	var buf bytes.Buffer
	w, err := zlib.NewWriterLevel(&buf, zlib.BestCompression)
	if err != nil {
		return baseURL
	}
	_, _ = w.Write(jsonBytes)
	w.Close()

	encoded := base64.RawURLEncoding.EncodeToString(buf.Bytes())
	return baseURL + "pako:" + encoded
}
