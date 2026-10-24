//! The parts of Go's `reflect` that tests read struct metadata through.
//!
//! A struct type's fields, each field's kind and its tag (`reflect.StructTag`). Rust has no run-time reflection, so a Go struct whose tags are read
//! through `reflect` carries an explicit `StructType` constant next to its Rust port, listing every Go field in
//! declaration order with its Go name, type kind and verbatim tag text.

use crate::strconv;

/// Go's `reflect.Kind`, limited to the kinds the ported struct metadata uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Int,
    String,
    Slice,
    Map,
    Struct,
    Pointer,
}

/// Go's `reflect.Type` of a struct field: its kind, plus the element type of a pointer (`Type.Elem`).
#[derive(Debug)]
pub struct Type {
    kind: Kind,
    elem: Option<&'static Type>,
}

impl Type {
    pub const BOOL: Type = Type::of(Kind::Bool);
    pub const INT: Type = Type::of(Kind::Int);
    pub const STRING: Type = Type::of(Kind::String);
    pub const SLICE: Type = Type::of(Kind::Slice);
    pub const MAP: Type = Type::of(Kind::Map);
    pub const STRUCT: Type = Type::of(Kind::Struct);

    /// A type of `kind` that is not a pointer.
    pub const fn of(kind: Kind) -> Type {
        Type { kind, elem: None }
    }

    /// Go's `reflect.PointerTo(elem)`.
    pub const fn pointer_to(elem: &'static Type) -> Type {
        Type { kind: Kind::Pointer, elem: Some(elem) }
    }

    /// Go's `Type.Kind()`.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Go's `Type.Elem()` of a pointer type; panics on other kinds, as Go does.
    pub fn elem(&self) -> &'static Type {
        match self.elem {
            Some(elem) => elem,
            None => panic!("reflect: Elem of invalid type"),
        }
    }
}

/// Go's `reflect.StructField`: the field's Go name, type and tag.
#[derive(Debug)]
pub struct StructField {
    pub name: &'static str,
    pub typ: &'static Type,
    pub tag: StructTag,
}

impl StructField {
    pub const fn new(name: &'static str, typ: &'static Type, tag: &'static str) -> StructField {
        StructField { name, typ, tag: StructTag(tag) }
    }
}

/// Go's `reflect.Type` of a struct (`reflect.TypeOf(T{})`): its fields in declaration order.
#[derive(Debug)]
pub struct StructType {
    pub fields: &'static [StructField],
}

impl StructType {
    /// Go's `Type.NumField()`.
    pub fn num_field(&self) -> usize {
        self.fields.len()
    }

    /// Go's `Type.Field(i)`.
    pub fn field(&self, i: usize) -> &'static StructField {
        &self.fields[i]
    }

    /// Go's `Type.FieldByName(name)` (no embedded fields to search).
    pub fn field_by_name(&self, name: &str) -> Option<&'static StructField> {
        self.fields.iter().find(|field| field.name == name)
    }
}

/// Go's `reflect.StructTag`: the tag string of a struct field, conventionally `key:"value"` pairs separated by
/// spaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructTag(pub &'static str);

impl StructTag {
    /// Go's `StructTag.Get(key)`: the value for `key`, or `""`.
    pub fn get(&self, key: &str) -> String {
        self.lookup(key).unwrap_or_default()
    }

    /// Go's `StructTag.Lookup(key)`, the same scan: a malformed tag ends the search.
    pub fn lookup(&self, key: &str) -> Option<String> {
        let mut tag = self.0.as_bytes();
        while !tag.is_empty() {
            // Skip leading space.
            let mut i = 0;
            while i < tag.len() && tag[i] == b' ' {
                i += 1;
            }
            tag = &tag[i..];
            if tag.is_empty() {
                break;
            }

            // Scan to colon. A space, a quote or a control character is a syntax error.
            i = 0;
            while i < tag.len()
                && tag[i] > b' '
                && tag[i] != b':'
                && tag[i] != b'"'
                && tag[i] != 0x7f
            {
                i += 1;
            }
            if i == 0 || i + 1 >= tag.len() || tag[i] != b':' || tag[i + 1] != b'"' {
                break;
            }
            let name = &tag[..i];
            tag = &tag[i + 1..];

            // Scan quoted string to find value.
            i = 1;
            while i < tag.len() && tag[i] != b'"' {
                if tag[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            if i >= tag.len() {
                break;
            }
            let qvalue = &tag[..=i];
            tag = &tag[i + 1..];

            if key.as_bytes() == name {
                let Ok(qvalue) = std::str::from_utf8(qvalue) else {
                    break;
                };
                return strconv::unquote(qvalue).ok();
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expectations from Go's reflect.StructTag documentation and behavior.
    #[test]
    fn struct_tag_lookup() {
        let tag = StructTag(r#"json:"name,omitzero" schema_default:"\"x\"" empty:"""#);
        assert_eq!(tag.get("json"), "name,omitzero");
        assert_eq!(tag.get("schema_default"), "\"x\"");
        assert_eq!(tag.lookup("empty"), Some(String::new()));
        assert_eq!(tag.lookup("missing"), None);
        assert_eq!(StructTag(r#"species:"gopher" color:"blue""#).get("color"), "blue");
        // A malformed pair stops the scan.
        assert_eq!(StructTag(r#"bad json:"x""#).lookup("json"), None);
    }
}
