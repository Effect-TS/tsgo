use std::ops::{BitAnd, BitOr, BitOrAssign};

use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

// go: internal/typeparser/object_literal.go:8 ObjectLiteralPropertyFlags c4c6025340f00291
// ObjectLiteralPropertyFlags tells what ObjectLiteralProperty learned about a
// property besides its initializer. None means the node is an object literal
// that certainly has no property with that name.
//
// Rust: a newtype over Go's `uint8` with the Go constants as associated consts and the bit operators Go uses.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct ObjectLiteralPropertyFlags(pub u8);

// go: internal/typeparser/object_literal.go:10 ObjectLiteralPropertyNone 2adcf90e0759a67d
impl ObjectLiteralPropertyFlags {
    pub const None: ObjectLiteralPropertyFlags = ObjectLiteralPropertyFlags(0);
    // The node is not an object literal (under any outer expressions).
    // Go: `1 << (iota - 1)` with iota 1 on this line.
    pub const NotObjectLiteral: ObjectLiteralPropertyFlags = ObjectLiteralPropertyFlags(1 << 0);
    // A member has the name.
    pub const Declared: ObjectLiteralPropertyFlags = ObjectLiteralPropertyFlags(1 << 1);
    // The last member with the name is a shorthand, method or accessor.
    pub const NotAssignment: ObjectLiteralPropertyFlags = ObjectLiteralPropertyFlags(1 << 2);
    // A spread after the last member with the name, or anywhere when no member has it.
    pub const MaybeSpread: ObjectLiteralPropertyFlags = ObjectLiteralPropertyFlags(1 << 3);
    // A computed name that is not a literal after the last member with the name,
    // or anywhere when no member has it.
    pub const MaybeComputed: ObjectLiteralPropertyFlags = ObjectLiteralPropertyFlags(1 << 4);
}

impl BitOr for ObjectLiteralPropertyFlags {
    type Output = ObjectLiteralPropertyFlags;
    fn bitor(self, rhs: ObjectLiteralPropertyFlags) -> ObjectLiteralPropertyFlags {
        ObjectLiteralPropertyFlags(self.0 | rhs.0)
    }
}

impl BitOrAssign for ObjectLiteralPropertyFlags {
    fn bitor_assign(&mut self, rhs: ObjectLiteralPropertyFlags) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for ObjectLiteralPropertyFlags {
    type Output = ObjectLiteralPropertyFlags;
    fn bitand(self, rhs: ObjectLiteralPropertyFlags) -> ObjectLiteralPropertyFlags {
        ObjectLiteralPropertyFlags(self.0 & rhs.0)
    }
}

// go: internal/typeparser/object_literal.go:32 ObjectLiteralProperty 5ff8c726f9388e8a
// ObjectLiteralProperty looks up a property of an object literal (under any
// parentheses, type assertions or satisfies). It returns the initializer the
// property is known to hold: the last property assignment with that name,
// provided nothing after it can override it. The flags tell why the
// initializer is nil: the name is absent (None), the value is not a plain
// assignment, or a later spread or unresolved computed name may set it.
// The initializer is non-nil exactly when the flags are Declared alone.
//
// Rust: Go's nil property-list check is the type system's.
pub fn object_literal_property(
    node: impl Into<Option<P<Node>>>,
    name: &str,
) -> (Option<P<Node>>, ObjectLiteralPropertyFlags) {
    let Some(node) = node.into() else {
        return (None, ObjectLiteralPropertyFlags::NotObjectLiteral);
    };
    let node =
        ets_shim::ast::skip_outer_expressions(node, ets_shim::ast::OuterExpressionKinds::All);
    if node.kind() != Kind::ObjectLiteralExpression {
        return (None, ObjectLiteralPropertyFlags::NotObjectLiteral);
    }
    let mut initializer = None;
    let mut flags = ObjectLiteralPropertyFlags::None;
    for &property in node.as_object_literal_expression().properties().nodes() {
        if property.kind() == Kind::SpreadAssignment {
            initializer = None;
            flags |= ObjectLiteralPropertyFlags::MaybeSpread;
            continue;
        }
        let Some(property_name) = property.name() else {
            continue;
        };
        match ets_shim::ast::try_get_text_of_property_name(property_name) {
            None => {
                initializer = None;
                flags |= ObjectLiteralPropertyFlags::MaybeComputed;
            }
            Some(text) if text != name => {}
            Some(_) if property.kind() == Kind::PropertyAssignment => {
                initializer = Some(property.as_property_assignment().initializer());
                flags = ObjectLiteralPropertyFlags::Declared;
            }
            Some(_) => {
                initializer = None;
                flags = ObjectLiteralPropertyFlags::Declared
                    | ObjectLiteralPropertyFlags::NotAssignment;
            }
        }
    }
    (initializer, flags)
}

// go: internal/typeparser/object_literal.go:76 ObjectLiteralPropertyInitializer ae6c3f5b550a46e3
// ObjectLiteralPropertyInitializer returns the initializer that a property of
// an object literal is known to hold, as ObjectLiteralProperty does, or nil
// when the value is absent or unknown.
pub fn object_literal_property_initializer(
    node: impl Into<Option<P<Node>>>,
    name: &str,
) -> Option<P<Node>> {
    let (initializer, _) = object_literal_property(node, name);
    initializer
}
