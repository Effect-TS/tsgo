use ets_go::sprintf;

use crate::*;

// go test: internal/layergraph/types_test.go:8 TestLayerGraphEdgeInfoMarshalJSON dd225fb980d0c072
#[test]
fn test_layer_graph_edge_info_marshal_json() {
    ets_go::testing::run(|t| {
        t.parallel();
        let edge = |relationship, argument_index, index| LayerGraphEdgeInfo {
            relationship,
            argument_index,
            index,
        };
        let tests = [
            (
                "call edge",
                edge(EdgeRelationship::Call, 0, 0),
                r#"{"relationship":"call","argumentIndex":0}"#,
            ),
            (
                "call edge with index 2",
                edge(EdgeRelationship::Call, 2, 0),
                r#"{"relationship":"call","argumentIndex":2}"#,
            ),
            ("pipe edge", edge(EdgeRelationship::Pipe, 0, 0), r#"{"relationship":"pipe"}"#),
            (
                "arrayLiteral edge",
                edge(EdgeRelationship::ArrayLiteral, 0, 0),
                r#"{"relationship":"arrayLiteral","index":0}"#,
            ),
            (
                "arrayLiteral edge with index 3",
                edge(EdgeRelationship::ArrayLiteral, 0, 3),
                r#"{"relationship":"arrayLiteral","index":3}"#,
            ),
            ("symbol edge", edge(EdgeRelationship::Symbol, 0, 0), r#"{"relationship":"symbol"}"#),
        ];

        for (name, edge, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                // Go: json.Marshal calls MarshalJSON, whose error is always nil.
                let b = edge.marshal_json();
                let got = String::from_utf8(b).expect("JSON is UTF-8");
                if got != expected {
                    t.error(sprintf!("got %s, want %s", got, expected));
                }
            });
        }
    });
}
