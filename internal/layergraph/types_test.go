package layergraph

import (
	"encoding/json"
	"testing"
)

func TestLayerGraphEdgeInfoMarshalJSON(t *testing.T) {
	tests := []struct {
		name     string
		edge     LayerGraphEdgeInfo
		expected string
	}{
		{
			name:     "call edge",
			edge:     LayerGraphEdgeInfo{Relationship: EdgeRelationshipCall, ArgumentIndex: 0},
			expected: `{"relationship":"call","argumentIndex":0}`,
		},
		{
			name:     "call edge with index 2",
			edge:     LayerGraphEdgeInfo{Relationship: EdgeRelationshipCall, ArgumentIndex: 2},
			expected: `{"relationship":"call","argumentIndex":2}`,
		},
		{
			name:     "pipe edge",
			edge:     LayerGraphEdgeInfo{Relationship: EdgeRelationshipPipe},
			expected: `{"relationship":"pipe"}`,
		},
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
		{
			name:     "symbol edge",
			edge:     LayerGraphEdgeInfo{Relationship: EdgeRelationshipSymbol},
			expected: `{"relationship":"symbol"}`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
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
