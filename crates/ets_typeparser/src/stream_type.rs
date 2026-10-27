use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/stream_type.go:6 StreamTypeId 88f1b56b5668989c
// StreamTypeId is the property key for Stream's variance struct.
pub const STREAM_TYPE_ID: &str = "~effect/Stream";

impl TypeParser<'_> {
    // go: internal/typeparser/stream_type.go:10 TypeParser.StreamType 126301c4deaee2d7
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
}
