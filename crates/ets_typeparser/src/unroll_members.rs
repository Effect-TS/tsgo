use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/unroll_members.go:7 TypeParser.UnrollUnionMembers
    // UnrollUnionMembers returns the constituent types of a union type,
    // or a single-element slice containing the type itself if it's not a union.
    //
    // Rust: Go's nil result (a nil type) is `None`.
    #[expect(clippy::unused_self, reason = "a TypeParser method in Go")]
    pub fn unroll_union_members(&self, t: impl Into<Option<P<Type>>>) -> Option<Vec<P<Type>>> {
        let t = t.into()?;
        if t.flags().intersects(TypeFlags::Union) {
            return Some(t.types().to_vec());
        }
        Some(vec![t])
    }
}
