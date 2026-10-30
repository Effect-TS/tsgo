package layergraph

import (
	"encoding/json"
	"testing"
)

func TestLayerGraphEdgeInfoMarshalJSON(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name     string
		edge     LayerGraphEdgeInfo
		expected string
	}{
		{
			name:     "arrayLiteral edge",
			edge:     LayerGraphEdgeInfo{Relationship: EdgeRelationshipArrayLiteral, Index: 0},
			expected: `{"relationship":"arrayLiteral","index":0}`,
		},
		{
			name:     "arrayLiteral edge with index 3",
			edge:     LayerGraphEdgeInfo{Relationship: EdgeRelationshipArrayLiteral, Index: 3},
			expected: `{"relationship":"arrayLiteral","index":3}`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			b, err := json.Marshal(tt.edge)
			if err != nil {
				t.Fatalf("MarshalJSON failed: %v", err)
			}
			got := string(b)
			if got != tt.expected {
				t.Errorf("got %s, want %s", got, tt.expected)
			}
		})
	}
}
