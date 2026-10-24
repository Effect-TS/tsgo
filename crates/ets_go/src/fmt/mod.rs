//! Go's `fmt`: `Sprintf`, `Fprintf`, `Errorf`, `Sprint`, `Sprintln`, `Fprint`, `Fprintln`.
//!
//! Go's verbs, flags, widths, precisions and argument indexes, and Go's error texts for bad verbs
//! (`%!d(string=x)`), missing (`%!s(MISSING)`) and extra (`%!(EXTRA int=1)`) arguments.
//!
//! The format strings are Go's, verbatim: `ets_go::sprintf!("%-10s|%5.2f|%v", name, x, items)`.
//!
//! Go formats arguments by reflection; here an argument implements [`Value`], which describes it as a
//! [`GoValue`]: the reflect kind, the Go type name (for `%T`, `%#v` and bad-verb texts), and the `Error()` /
//! `String()` / `GoString()` methods Go would find in its method set ([`GoValue::Methods`]). The printer
//! (`print.rs`, `format.rs`) is Go's `fmt/print.go` and `fmt/format.go` ported over that description, so
//! `%v` of slices (`[a b]`), maps (`map[a:1 b:2]`, keys sorted as `internal/fmtsort` does), structs
//! (`{a b}`, `%+v` `{A:a B:b}`, `%#v` `pkg.T{A:"a", B:2}`), pointers (`&{...}` at the top level), nil
//! (`<nil>`), errors (`Error()`), and floats (`%v` is `strconv.FormatFloat(f, 'g', -1, 64)`) are Go's.
//!
//! A ported type gets a `Value` impl that says what Go would see: [`impl_go_struct!`] for structs,
//! [`GoValue::stringer`] / [`GoValue::error_value`] for types with `String()` / `Error()` methods (only when
//! the Go method set of the argument's type has them: a pointer-receiver `String()` is not called on a value).
//!
//! Not supported: `fmt.Formatter` implementations (the ported code has none) and complex numbers.

mod format;
mod print;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::hash::BuildHasher;
use std::sync::Arc;

use crate::Error;

pub use print::{append_f, errorf, fprint, fprintf, fprintln, sprint, sprintf, sprintln};

/// A Go type name as `reflect.Type.String()` prints it (`int`, `[]string`, `*ledger.Key`, `map[string]int`).
pub type TypeName = Cow<'static, str>;

/// Go's `reflect.Kind`, as far as formatting needs it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Bool,
    Int,
    Uint,
    /// `uint8` / `byte`: slices and arrays of it format as bytes for `%s %q %x %X`.
    Uint8,
    Float,
    String,
    Slice,
    Array,
    Map,
    Struct,
    Pointer,
    Interface,
    Func,
    Chan,
}

/// A Go static type: its name and kind. Used for nil values and empty containers, where there is no value
/// to describe.
#[derive(Clone, Debug)]
pub struct GoType {
    pub name: TypeName,
    pub kind: Kind,
}

impl GoType {
    pub fn new(name: impl Into<TypeName>, kind: Kind) -> GoType {
        GoType { name: name.into(), kind }
    }

    /// The zero value Go prints for a nil of this type (`Option<T>::None`): a nil pointer for
    /// non-nilable kinds (Go's `*T`), otherwise a nil of the kind itself.
    pub fn nil(&self) -> GoValue<'static> {
        match self.kind {
            Kind::Pointer => GoValue::Pointer { ty: self.name.clone(), addr: 0, elem: None },
            Kind::Slice => {
                GoValue::Slice { ty: self.name.clone(), elem_kind: Kind::Interface, elems: None }
            }
            Kind::Map => GoValue::Map { ty: self.name.clone(), entries: None },
            Kind::Interface => GoValue::Interface { ty: self.name.clone(), elem: None },
            Kind::Func | Kind::Chan => GoValue::Opaque { ty: self.name.clone(), addr: 0 },
            _ => GoValue::Pointer { ty: format!("*{}", self.name).into(), addr: 0, elem: None },
        }
    }
}

/// A struct field as `fmt` sees it.
#[derive(Clone, Debug)]
pub struct Field<'a> {
    /// The Go field name (the type name for an embedded field).
    pub name: Cow<'static, str>,
    pub value: GoValue<'a>,
}

impl<'a> Field<'a> {
    pub fn new(name: impl Into<Cow<'static, str>>, value: GoValue<'a>) -> Field<'a> {
        Field { name: name.into(), value }
    }

    /// Exported fields are reachable by reflection's `Interface()`, so `fmt` calls their methods; it does
    /// not for unexported ones.
    pub(crate) fn exported(&self) -> bool {
        self.name.chars().next().is_some_and(char::is_uppercase)
    }
}

/// What Go's reflection sees of a value passed to `fmt`.
#[derive(Clone, Debug)]
pub enum GoValue<'a> {
    /// Go's untyped `nil` argument (a nil interface), reflect's invalid `Value`.
    Nil,
    Bool(bool, TypeName),
    Int(i64, TypeName),
    Uint(u64, TypeName, Kind),
    /// A float and its size in bits (32 or 64).
    Float(f64, u32, TypeName),
    /// A Go string (any bytes).
    Str(Cow<'a, [u8]>, TypeName),
    /// A slice; `elems: None` is a nil slice.
    Slice {
        ty: TypeName,
        elem_kind: Kind,
        elems: Option<Vec<GoValue<'a>>>,
    },
    Array {
        ty: TypeName,
        elem_kind: Kind,
        elems: Vec<GoValue<'a>>,
    },
    /// A map's entries in any order (the printer sorts them); `None` is a nil map.
    Map {
        ty: TypeName,
        entries: Option<Vec<(GoValue<'a>, GoValue<'a>)>>,
    },
    Struct {
        ty: TypeName,
        fields: Vec<Field<'a>>,
    },
    /// A pointer; `addr == 0` is nil. `elem` is the pointee when it should be printed (`&{...}` at the top
    /// level); `None` with a nonzero address prints the address.
    Pointer {
        ty: TypeName,
        addr: usize,
        elem: Option<Box<GoValue<'a>>>,
    },
    /// An interface-typed slot (struct field, slice element) holding nil. A non-nil interface value is
    /// represented by its dynamic value directly.
    Interface {
        ty: TypeName,
        elem: Option<Box<GoValue<'a>>>,
    },
    /// A func or chan (printed as its address).
    Opaque {
        ty: TypeName,
        addr: usize,
    },
    /// A value whose Go method set has `Error()`, `String()` and/or `GoString()`: the method results and
    /// the value itself (what reflection sees for verbs that do not call them).
    Methods {
        value: Box<GoValue<'a>>,
        error: Option<String>,
        string: Option<String>,
        go_string: Option<String>,
    },
}

impl<'a> GoValue<'a> {
    /// A value with a `String() string` method (Go's `fmt.Stringer`).
    pub fn stringer(text: String, value: GoValue<'a>) -> GoValue<'a> {
        GoValue::Methods {
            value: Box::new(value),
            error: None,
            string: Some(text),
            go_string: None,
        }
    }

    /// A value with an `Error() string` method.
    pub fn error_value(text: String, value: GoValue<'a>) -> GoValue<'a> {
        GoValue::Methods {
            value: Box::new(value),
            error: Some(text),
            string: None,
            go_string: None,
        }
    }

    /// A Go string from a Rust string.
    pub fn str(s: &'a str) -> GoValue<'a> {
        GoValue::Str(Cow::Borrowed(s.as_bytes()), Cow::Borrowed("string"))
    }

    /// A struct value.
    pub fn struct_(ty: impl Into<TypeName>, fields: Vec<Field<'a>>) -> GoValue<'a> {
        GoValue::Struct { ty: ty.into(), fields }
    }

    /// A pointer to `elem` (Go's `&v`), with a stand-in address. The method set of `*T` includes `T`'s
    /// value-receiver methods, so the pointer has `elem`'s `Error()` / `String()` too.
    pub fn pointer_to(ty: impl Into<TypeName>, addr: usize, elem: GoValue<'a>) -> GoValue<'a> {
        let methods = match &elem {
            GoValue::Methods { error, string, go_string, .. } => {
                Some((error.clone(), string.clone(), go_string.clone()))
            }
            _ => None,
        };
        let pointer =
            GoValue::Pointer { ty: ty.into(), addr: addr.max(1), elem: Some(Box::new(elem)) };
        match methods {
            Some((error, string, go_string)) => {
                GoValue::Methods { value: Box::new(pointer), error, string, go_string }
            }
            None => pointer,
        }
    }

    /// `reflect.TypeOf(v).String()`.
    pub fn type_name(&self) -> Cow<'_, str> {
        match self {
            GoValue::Nil => Cow::Borrowed("<nil>"),
            GoValue::Bool(_, ty)
            | GoValue::Int(_, ty)
            | GoValue::Uint(_, ty, _)
            | GoValue::Float(_, _, ty)
            | GoValue::Str(_, ty)
            | GoValue::Slice { ty, .. }
            | GoValue::Array { ty, .. }
            | GoValue::Map { ty, .. }
            | GoValue::Struct { ty, .. }
            | GoValue::Pointer { ty, .. }
            | GoValue::Interface { ty, .. }
            | GoValue::Opaque { ty, .. } => Cow::Borrowed(ty),
            GoValue::Methods { value, .. } => value.type_name(),
        }
    }

    /// The same value without borrowed parts.
    pub fn into_owned_value(self) -> GoValue<'static> {
        match self {
            GoValue::Nil => GoValue::Nil,
            GoValue::Bool(v, ty) => GoValue::Bool(v, ty),
            GoValue::Int(v, ty) => GoValue::Int(v, ty),
            GoValue::Uint(v, ty, k) => GoValue::Uint(v, ty, k),
            GoValue::Float(v, size, ty) => GoValue::Float(v, size, ty),
            GoValue::Str(s, ty) => GoValue::Str(Cow::Owned(s.into_owned()), ty),
            GoValue::Slice { ty, elem_kind, elems } => GoValue::Slice {
                ty,
                elem_kind,
                elems: elems.map(|e| e.into_iter().map(GoValue::into_owned_value).collect()),
            },
            GoValue::Array { ty, elem_kind, elems } => GoValue::Array {
                ty,
                elem_kind,
                elems: elems.into_iter().map(GoValue::into_owned_value).collect(),
            },
            GoValue::Map { ty, entries } => GoValue::Map {
                ty,
                entries: entries.map(|e| {
                    e.into_iter()
                        .map(|(k, v)| (k.into_owned_value(), v.into_owned_value()))
                        .collect()
                }),
            },
            GoValue::Struct { ty, fields } => GoValue::Struct {
                ty,
                fields: fields
                    .into_iter()
                    .map(|f| Field { name: f.name, value: f.value.into_owned_value() })
                    .collect(),
            },
            GoValue::Pointer { ty, addr, elem } => {
                GoValue::Pointer { ty, addr, elem: elem.map(|e| Box::new(e.into_owned_value())) }
            }
            GoValue::Interface { ty, elem } => {
                GoValue::Interface { ty, elem: elem.map(|e| Box::new(e.into_owned_value())) }
            }
            GoValue::Opaque { ty, addr } => GoValue::Opaque { ty, addr },
            GoValue::Methods { value, error, string, go_string } => GoValue::Methods {
                value: Box::new(value.into_owned_value()),
                error,
                string,
                go_string,
            },
        }
    }

    /// The value without its methods (what reflection's `Kind` switch sees).
    pub(crate) fn underlying(&self) -> &GoValue<'a> {
        match self {
            GoValue::Methods { value, .. } => value.underlying(),
            v => v,
        }
    }
}

/// An argument to the `fmt` functions: Go's `any`, described for formatting.
pub trait Value {
    /// What `fmt`'s reflection sees of the value.
    fn go_value(&self) -> GoValue<'_>;

    /// The value as an `error`, when it is one (what `%w` wraps).
    fn as_error(&self) -> Option<Error> {
        None
    }
}

/// The Go static type of a Rust type, for nils and empty containers (`[]T`, `map[K]V`, `*T`).
pub trait GoTyped {
    fn go_type() -> GoType;
}

impl<T: Value + ?Sized> Value for &T {
    fn go_value(&self) -> GoValue<'_> {
        (**self).go_value()
    }

    fn as_error(&self) -> Option<Error> {
        (**self).as_error()
    }
}

impl<T: GoTyped + ?Sized> GoTyped for &T {
    fn go_type() -> GoType {
        T::go_type()
    }
}

impl Value for GoValue<'_> {
    fn go_value(&self) -> GoValue<'_> {
        self.clone()
    }
}

macro_rules! impl_int {
    ($($t:ty => $name:literal),*) => {$(
        impl Value for $t {
            fn go_value(&self) -> GoValue<'_> {
                GoValue::Int(*self as i64, Cow::Borrowed($name))
            }
        }

        impl GoTyped for $t {
            fn go_type() -> GoType {
                GoType::new($name, Kind::Int)
            }
        }
    )*};
}

// Go's `int` is 64-bit and maps to Rust's `i64` (docs/PORTING.md, "Types"), so `i64` prints as `int`; Rust's
// `usize` / `isize` are what Go's `int` lengths and indexes become.
impl_int!(i8 => "int8", i16 => "int16", i32 => "int32", i64 => "int", isize => "int", usize => "int");

macro_rules! impl_uint {
    ($($t:ty => $name:literal, $kind:expr),*) => {$(
        impl Value for $t {
            fn go_value(&self) -> GoValue<'_> {
                GoValue::Uint(u64::from(*self), Cow::Borrowed($name), $kind)
            }
        }

        impl GoTyped for $t {
            fn go_type() -> GoType {
                GoType::new($name, $kind)
            }
        }
    )*};
}

impl_uint!(u8 => "uint8", Kind::Uint8, u16 => "uint16", Kind::Uint, u32 => "uint32", Kind::Uint, u64 => "uint64", Kind::Uint);

impl Value for f64 {
    fn go_value(&self) -> GoValue<'_> {
        GoValue::Float(*self, 64, Cow::Borrowed("float64"))
    }
}

impl GoTyped for f64 {
    fn go_type() -> GoType {
        GoType::new("float64", Kind::Float)
    }
}

impl Value for f32 {
    fn go_value(&self) -> GoValue<'_> {
        GoValue::Float(f64::from(*self), 32, Cow::Borrowed("float32"))
    }
}

impl GoTyped for f32 {
    fn go_type() -> GoType {
        GoType::new("float32", Kind::Float)
    }
}

impl Value for bool {
    fn go_value(&self) -> GoValue<'_> {
        GoValue::Bool(*self, Cow::Borrowed("bool"))
    }
}

impl GoTyped for bool {
    fn go_type() -> GoType {
        GoType::new("bool", Kind::Bool)
    }
}

/// Go's `rune` (an `int32`).
impl Value for char {
    fn go_value(&self) -> GoValue<'_> {
        GoValue::Int(i64::from(u32::from(*self)), Cow::Borrowed("int32"))
    }
}

impl GoTyped for char {
    fn go_type() -> GoType {
        GoType::new("int32", Kind::Int)
    }
}

impl Value for str {
    fn go_value(&self) -> GoValue<'_> {
        GoValue::str(self)
    }
}

impl GoTyped for str {
    fn go_type() -> GoType {
        GoType::new("string", Kind::String)
    }
}

impl Value for String {
    fn go_value(&self) -> GoValue<'_> {
        GoValue::str(self)
    }
}

impl GoTyped for String {
    fn go_type() -> GoType {
        GoType::new("string", Kind::String)
    }
}

impl Value for Cow<'_, str> {
    fn go_value(&self) -> GoValue<'_> {
        GoValue::str(self)
    }
}

impl GoTyped for Cow<'_, str> {
    fn go_type() -> GoType {
        GoType::new("string", Kind::String)
    }
}

impl<T: Value + GoTyped> Value for [T] {
    fn go_value(&self) -> GoValue<'_> {
        let elem = T::go_type();
        GoValue::Slice {
            ty: format!("[]{}", elem.name).into(),
            elem_kind: elem.kind,
            elems: Some(self.iter().map(Value::go_value).collect()),
        }
    }
}

impl<T: GoTyped> GoTyped for [T] {
    fn go_type() -> GoType {
        GoType::new(format!("[]{}", T::go_type().name), Kind::Slice)
    }
}

impl<T: Value + GoTyped> Value for Vec<T> {
    fn go_value(&self) -> GoValue<'_> {
        self.as_slice().go_value()
    }
}

impl<T: GoTyped> GoTyped for Vec<T> {
    fn go_type() -> GoType {
        GoType::new(format!("[]{}", T::go_type().name), Kind::Slice)
    }
}

impl<T: Value + GoTyped, const N: usize> Value for [T; N] {
    fn go_value(&self) -> GoValue<'_> {
        let elem = T::go_type();
        GoValue::Array {
            ty: format!("[{N}]{}", elem.name).into(),
            elem_kind: elem.kind,
            elems: self.iter().map(Value::go_value).collect(),
        }
    }
}

impl<T: GoTyped, const N: usize> GoTyped for [T; N] {
    fn go_type() -> GoType {
        GoType::new(format!("[{N}]{}", T::go_type().name), Kind::Array)
    }
}

/// `None` is the Go nil of `T`'s type (a nil pointer, slice, map or interface); `Some(v)` is `v`.
impl<T: Value + GoTyped> Value for Option<T> {
    fn go_value(&self) -> GoValue<'_> {
        match self {
            Some(v) => v.go_value(),
            None => T::go_type().nil(),
        }
    }

    fn as_error(&self) -> Option<Error> {
        self.as_ref().and_then(Value::as_error)
    }
}

impl<T: GoTyped> GoTyped for Option<T> {
    fn go_type() -> GoType {
        T::go_type()
    }
}

/// `Box<T>` and `Arc<T>` stand for Go's `*T` (docs/PORTING.md, "Types").
impl<T: Value + GoTyped> Value for Box<T> {
    fn go_value(&self) -> GoValue<'_> {
        let ty = format!("*{}", T::go_type().name);
        GoValue::pointer_to(ty, std::ptr::from_ref::<T>(&**self).addr(), (**self).go_value())
    }
}

impl<T: GoTyped> GoTyped for Box<T> {
    fn go_type() -> GoType {
        GoType::new(format!("*{}", T::go_type().name), Kind::Pointer)
    }
}

impl<T: Value + GoTyped> Value for Arc<T> {
    fn go_value(&self) -> GoValue<'_> {
        let ty = format!("*{}", T::go_type().name);
        GoValue::pointer_to(ty, Arc::as_ptr(self).addr(), (**self).go_value())
    }
}

impl<T: GoTyped> GoTyped for Arc<T> {
    fn go_type() -> GoType {
        GoType::new(format!("*{}", T::go_type().name), Kind::Pointer)
    }
}

fn map_type<K: GoTyped, V: GoTyped>() -> GoType {
    GoType::new(format!("map[{}]{}", K::go_type().name, V::go_type().name), Kind::Map)
}

fn map_value<'a, K: Value + GoTyped + 'a, V: Value + GoTyped + 'a>(
    entries: impl Iterator<Item = (&'a K, &'a V)>,
) -> GoValue<'a> {
    GoValue::Map {
        ty: map_type::<K, V>().name,
        entries: Some(entries.map(|(k, v)| (k.go_value(), v.go_value())).collect()),
    }
}

#[expect(
    clippy::disallowed_types,
    reason = "covers FxHashMap, which is std's HashMap with another hasher"
)]
impl<K: Value + GoTyped, V: Value + GoTyped, S: BuildHasher> Value
    for std::collections::HashMap<K, V, S>
{
    fn go_value(&self) -> GoValue<'_> {
        map_value(self.iter())
    }
}

#[expect(
    clippy::disallowed_types,
    reason = "covers FxHashMap, which is std's HashMap with another hasher"
)]
impl<K: GoTyped, V: GoTyped, S: BuildHasher> GoTyped for std::collections::HashMap<K, V, S> {
    fn go_type() -> GoType {
        map_type::<K, V>()
    }
}

impl<K: Value + GoTyped, V: Value + GoTyped> Value for BTreeMap<K, V> {
    fn go_value(&self) -> GoValue<'_> {
        map_value(self.iter())
    }
}

impl<K: GoTyped, V: GoTyped> GoTyped for BTreeMap<K, V> {
    fn go_type() -> GoType {
        map_type::<K, V>()
    }
}

/// Go's set idiom, `map[K]struct{}`.
#[expect(
    clippy::disallowed_types,
    reason = "covers FxHashSet, which is std's HashSet with another hasher"
)]
impl<K: Value + GoTyped, S: BuildHasher> Value for std::collections::HashSet<K, S> {
    fn go_value(&self) -> GoValue<'_> {
        let empty = || GoValue::Struct { ty: Cow::Borrowed("struct {}"), fields: Vec::new() };
        GoValue::Map {
            ty: format!("map[{}]struct {{}}", K::go_type().name).into(),
            entries: Some(self.iter().map(|k| (k.go_value(), empty())).collect()),
        }
    }
}

#[expect(
    clippy::disallowed_types,
    reason = "covers FxHashSet, which is std's HashSet with another hasher"
)]
impl<K: GoTyped, S: BuildHasher> GoTyped for std::collections::HashSet<K, S> {
    fn go_type() -> GoType {
        GoType::new(format!("map[{}]struct {{}}", K::go_type().name), Kind::Map)
    }
}

/// An error prints its `Error()` text for `%v %s %q %x %X` (and `%w` in `Errorf`); other verbs see the
/// pointer behind it.
impl Value for Error {
    fn go_value(&self) -> GoValue<'_> {
        let e = self.as_dyn();
        let value = GoValue::Pointer {
            ty: e.type_name(),
            addr: self.addr().max(1),
            elem: e.go_value().map(Box::new),
        };
        GoValue::error_value(e.error(), value)
    }

    fn as_error(&self) -> Option<Error> {
        Some(self.clone())
    }
}

impl GoTyped for Error {
    fn go_type() -> GoType {
        GoType::new("error", Kind::Interface)
    }
}

/// Implements [`Value`] and [`GoTyped`] for a ported Go struct.
///
/// Give its Go type name and its fields in declaration order with their Go names (exported fields start
/// with an uppercase letter; `fmt` calls `Error()` / `String()` only on those).
///
/// ```
/// struct ProgramKey { root: String, generation: i64 }
/// ets_go::impl_go_struct!(ProgramKey = "ledger.ProgramKey" { "Root": root, "Generation": generation });
/// let key = ProgramKey { root: "/w".into(), generation: 3 };
/// assert_eq!(ets_go::sprintf!("%v %+v", key, key), "{/w 3} {Root:/w Generation:3}");
/// ```
#[macro_export]
macro_rules! impl_go_struct {
    ($ty:ty = $name:literal { $($go:literal : $field:ident),* $(,)? }) => {
        impl $crate::fmt::Value for $ty {
            fn go_value(&self) -> $crate::fmt::GoValue<'_> {
                $crate::fmt::GoValue::struct_(
                    $name,
                    ::std::vec![$($crate::fmt::Field::new($go, $crate::fmt::Value::go_value(&self.$field))),*],
                )
            }
        }

        impl $crate::fmt::GoTyped for $ty {
            fn go_type() -> $crate::fmt::GoType {
                $crate::fmt::GoType::new($name, $crate::fmt::Kind::Struct)
            }
        }
    };
}

/// Go's `fmt.Sprintf(format, args...)`.
#[macro_export]
macro_rules! sprintf {
    ($format:expr $(, $arg:expr)* $(,)?) => {
        $crate::fmt::sprintf($format, &[$(&$arg as &dyn $crate::fmt::Value),*])
    };
}

/// Go's `fmt.Errorf(format, args...)`: an [`Error`](crate::Error) that wraps the `%w` operands.
#[macro_export]
macro_rules! errorf {
    ($format:expr $(, $arg:expr)* $(,)?) => {
        $crate::fmt::errorf($format, &[$(&$arg as &dyn $crate::fmt::Value),*])
    };
}

/// Go's `fmt.Fprintf(w, format, args...)` on a `std::io::Write`; returns `io::Result<usize>`.
#[macro_export]
macro_rules! fprintf {
    ($w:expr, $format:expr $(, $arg:expr)* $(,)?) => {
        $crate::fmt::fprintf($w, $format, &[$(&$arg as &dyn $crate::fmt::Value),*])
    };
}

/// Go's `fmt.Sprint(args...)`.
#[macro_export]
macro_rules! sprint {
    ($($arg:expr),* $(,)?) => {
        $crate::fmt::sprint(&[$(&$arg as &dyn $crate::fmt::Value),*])
    };
}

/// Go's `fmt.Sprintln(args...)`.
#[macro_export]
macro_rules! sprintln {
    ($($arg:expr),* $(,)?) => {
        $crate::fmt::sprintln(&[$(&$arg as &dyn $crate::fmt::Value),*])
    };
}

/// Go's `fmt.Fprint(w, args...)`.
#[macro_export]
macro_rules! fprint {
    ($w:expr $(, $arg:expr)* $(,)?) => {
        $crate::fmt::fprint($w, &[$(&$arg as &dyn $crate::fmt::Value),*])
    };
}

/// Go's `fmt.Fprintln(w, args...)`.
#[macro_export]
macro_rules! fprintln {
    ($w:expr $(, $arg:expr)* $(,)?) => {
        $crate::fmt::fprintln($w, &[$(&$arg as &dyn $crate::fmt::Value),*])
    };
}
