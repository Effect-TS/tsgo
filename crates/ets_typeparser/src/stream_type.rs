use ets_shim::ast::Node;
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/stream_type.go:9 StreamTypeId 88f1b56b5668989c
// StreamTypeId is the property key for Stream's variance struct.
pub const STREAM_TYPE_ID: &str = "~effect/Stream";

impl TypeParser<'_> {
    // go: internal/typeparser/stream_type.go:13 TypeParser.StreamType 126301c4deaee2d7
    // StreamType parses a Stream type and extracts A, E, R parameters.
    // For v3, Stream carries the Effect variance struct, so this delegates to EffectType.
    pub fn stream_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Effect> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.stream_type,
            t,
            |tp| {
                if tp.detect_effect_version() != EffectMajorVersion::V4 {
                    return tp.effect_type(t);
                }

                let variance_struct_type = tp.get_type_of_property_by_name(t, STREAM_TYPE_ID)?;
                tp.parse_variance_struct(Some(variance_struct_type))
            },
        )
    }

    // go: internal/typeparser/stream_type.go:33 TypeParser.StreamTypeAtLocation b1dce0a3e52c10ff
    // StreamTypeAtLocation parses the type of node as a Stream type.
    // Returns nil if the type is not a Stream, without computing the type when
    // nodeCannotBeEffectLibraryType rules node out.
    pub fn stream_type_at_location(&mut self, node: impl Into<Option<P<Node>>>) -> Option<Effect> {
        let t = self.effect_library_type_at_location(node);
        self.stream_type(t)
    }
}
