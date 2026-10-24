//! Go's `error` values and the `errors` package: `errors.New`, `Is`, `As` / `AsType`, `Join`, `Unwrap`.
//!
//! [`Error`] is Go's `error` interface value: a cheap, cloneable handle to something that implements
//! [`GoError`]. Go's `nil` error is `Option<Error>::None` (or the `Ok` side of a `Result`).
//!
//! - `errors.New("x")` -> [`new`]; a package-level `var ErrX = errors.New("x")` -> [`sentinel!`], a `static`
//!   whose identity `errors.Is` compares, exactly like Go's pointer comparison.
//! - `fmt.Errorf("...: %w", err)` -> `ets_go::errorf!("...: %w", err)`, which keeps the chain
//!   ([`crate::fmt::errorf`]).
//! - A typed error (`type QueryError struct{...}` with an `Error()` method) implements [`GoError`];
//!   `errors.As(err, &target)` / `errors.AsType[*QueryError](err)` -> [`as_type::<QueryError>`](as_type).
//!
//! `Error()` texts are Go's byte for byte; `Display` prints the same text.

use std::any::{Any, TypeId};
use std::borrow::Cow;
use std::sync::Arc;

/// What a Go error's `Unwrap` method returns.
pub enum Unwrapped<'a> {
    /// No `Unwrap` method, or `Unwrap() error` returned nil.
    None,
    /// `Unwrap() error`.
    One(&'a Error),
    /// `Unwrap() []error`.
    Many(&'a [Error]),
}

/// A Go type that implements `error`: `Error() string` plus the optional methods `errors` looks for.
pub trait GoError: Any + Send + Sync {
    /// Go's `Error() string`.
    fn error(&self) -> String;

    /// Go's `%T` of the error value, e.g. `*errors.errorString`, `*protocol.QueryError`.
    fn type_name(&self) -> Cow<'static, str>;

    /// Go's `Unwrap() error` / `Unwrap() []error`.
    fn unwrap(&self) -> Unwrapped<'_> {
        Unwrapped::None
    }

    /// Go's optional `Is(target error) bool` method.
    fn is(&self, target: &Error) -> bool {
        let _ = target;
        false
    }

    /// Go's `==` between two error values of this type when the type is a comparable value type (a struct
    /// without pointer receivers). Identity (same allocation, same static) is always checked first; override
    /// this only for value-type errors whose distinct instances compare equal in Go.
    fn equal(&self, other: &dyn GoError) -> bool {
        let _ = other;
        false
    }

    /// The value behind the error for `fmt` verbs that do not call `Error()` (`%d`, `%t`, ...) and for an
    /// error reached through an unexported struct field. `None` prints it as a pointer, which is what Go
    /// does for the usual pointer-receiver error types.
    fn go_value(&self) -> Option<crate::fmt::GoValue<'_>> {
        None
    }
}

/// Go's `error` interface value (non-nil). Clone is cheap (a reference count or a static).
#[derive(Clone)]
pub struct Error(Repr);

#[derive(Clone)]
enum Repr {
    Static(&'static dyn GoError),
    Shared(Arc<dyn GoError>),
}

impl Error {
    /// Wraps a static error value; used by [`sentinel!`].
    pub const fn from_static(e: &'static dyn GoError) -> Error {
        Error(Repr::Static(e))
    }

    /// Wraps a typed error value (Go's implicit conversion of a `*T` to `error`).
    pub fn new<E: GoError>(e: E) -> Error {
        Error(Repr::Shared(Arc::new(e)))
    }

    /// Wraps an already shared error value.
    pub fn from_arc(e: Arc<dyn GoError>) -> Error {
        Error(Repr::Shared(e))
    }

    /// The error value behind the interface.
    pub fn as_dyn(&self) -> &dyn GoError {
        match &self.0 {
            Repr::Static(e) => *e,
            Repr::Shared(e) => &**e,
        }
    }

    /// Go's `err.Error()`.
    pub fn error(&self) -> String {
        self.as_dyn().error()
    }

    /// Go's `%T` of the dynamic value.
    pub fn type_name(&self) -> Cow<'static, str> {
        self.as_dyn().type_name()
    }

    /// The dynamic value as `T` (Go's type assertion `err.(*T)`), without walking the chain.
    pub fn downcast_ref<T: GoError>(&self) -> Option<&T> {
        let any: &dyn Any = self.as_dyn();
        any.downcast_ref::<T>()
    }

    /// Go's `err == target` on two interface values.
    pub fn same(&self, target: &Error) -> bool {
        same(self.as_dyn(), target.as_dyn())
    }

    /// The address of the dynamic value, for `%p` and Go's pointer printing.
    pub(crate) fn addr(&self) -> usize {
        std::ptr::from_ref::<dyn GoError>(self.as_dyn()).cast::<u8>() as usize
    }
}

fn same(a: &dyn GoError, b: &dyn GoError) -> bool {
    let (ta, tb) = ((a as &dyn Any).type_id(), (b as &dyn Any).type_id());
    if ta != tb {
        return false;
    }
    // Zero-sized error types (Go's `type deadlineExceededError struct{}`) are equal by value.
    std::ptr::addr_eq(std::ptr::from_ref(a), std::ptr::from_ref(b))
        || size_of_val(a) == 0
        || a.equal(b)
}

impl<E: GoError> From<E> for Error {
    fn from(e: E) -> Error {
        Error::new(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.error())
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({:?})", self.type_name(), self.error())
    }
}

/// Go's `*errors.errorString`, what `errors.New` returns.
pub struct ErrorString {
    s: Cow<'static, str>,
}

impl ErrorString {
    /// A static `errorString`, for [`sentinel!`].
    pub const fn new_static(s: &'static str) -> ErrorString {
        ErrorString { s: Cow::Borrowed(s) }
    }
}

impl GoError for ErrorString {
    fn error(&self) -> String {
        self.s.to_string()
    }

    fn type_name(&self) -> Cow<'static, str> {
        Cow::Borrowed("*errors.errorString")
    }

    fn go_value(&self) -> Option<crate::fmt::GoValue<'_>> {
        Some(crate::fmt::GoValue::struct_(
            "errors.errorString",
            vec![crate::fmt::Field::new("s", crate::fmt::GoValue::str(&self.s))],
        ))
    }
}

/// Go's `errors.New(text)`: a new error, distinct from every other (`errors.Is` compares identity).
pub fn new(text: impl Into<String>) -> Error {
    Error::new(ErrorString { s: Cow::Owned(text.into()) })
}

/// Declares a sentinel error: Go's package-level `var ErrX = errors.New("text")`.
///
/// ```
/// ets_go::sentinel! {
///     /// Reported when the key is absent.
///     pub static ErrNotFound = "not found";
/// }
/// let err = ets_go::errorf!("lookup %q: %w", "k", ErrNotFound);
/// assert!(ets_go::errors::is(&err, &ErrNotFound));
/// assert_eq!(err.error(), r#"lookup "k": not found"#);
/// ```
#[macro_export]
macro_rules! sentinel {
    ($($(#[$meta:meta])* $vis:vis static $name:ident = $text:expr;)+) => {$(
        $(#[$meta])*
        $vis static $name: $crate::Error = {
            static ERROR_STRING: $crate::errors::ErrorString = $crate::errors::ErrorString::new_static($text);
            $crate::Error::from_static(&ERROR_STRING)
        };
    )+};
}

sentinel! {
    /// Go's `errors.ErrUnsupported`.
    pub static ErrUnsupported = "unsupported operation";
}

/// Go's `errors.Is(err, target)`: whether any error in `err`'s tree (depth first, `Unwrap() []error`
/// branches in order) equals `target` or has an `Is` method that says so.
pub fn is(err: &Error, target: &Error) -> bool {
    let mut err = err;
    loop {
        if err.same(target) {
            return true;
        }
        if err.as_dyn().is(target) {
            return true;
        }
        match err.as_dyn().unwrap() {
            Unwrapped::One(next) => err = next,
            Unwrapped::Many(errs) => return errs.iter().any(|e| is(e, target)),
            Unwrapped::None => return false,
        }
    }
}

/// Go's `errors.Is` with a possibly nil `err` and `target`: `err == target` when either is nil.
pub fn is_opt(err: Option<&Error>, target: Option<&Error>) -> bool {
    match (err, target) {
        (Some(err), Some(target)) => is(err, target),
        (None, None) => true,
        _ => false,
    }
}

/// Go's `errors.AsType[*T](err)` / `errors.As(err, &target)`: the first error in `err`'s tree whose dynamic
/// type is `T`.
pub fn as_type<T: GoError>(err: &Error) -> Option<&T> {
    let mut err = err;
    loop {
        if let Some(t) = err.downcast_ref::<T>() {
            return Some(t);
        }
        match err.as_dyn().unwrap() {
            Unwrapped::One(next) => err = next,
            Unwrapped::Many(errs) => return errs.iter().find_map(|e| as_type::<T>(e)),
            Unwrapped::None => return None,
        }
    }
}

/// Go's `errors.Unwrap(err)`: the result of `Unwrap() error`; `None` for `Unwrap() []error` too.
pub fn unwrap(err: &Error) -> Option<Error> {
    match err.as_dyn().unwrap() {
        Unwrapped::One(e) => Some(e.clone()),
        Unwrapped::Many(_) | Unwrapped::None => None,
    }
}

/// Go's `*errors.joinError`, what `errors.Join` returns.
pub struct JoinError {
    errs: Vec<Error>,
}

impl GoError for JoinError {
    fn error(&self) -> String {
        if self.errs.len() == 1 {
            return self.errs[0].error();
        }
        let mut b = self.errs[0].error();
        for err in &self.errs[1..] {
            b.push('\n');
            b.push_str(&err.error());
        }
        b
    }

    fn type_name(&self) -> Cow<'static, str> {
        Cow::Borrowed("*errors.joinError")
    }

    fn unwrap(&self) -> Unwrapped<'_> {
        Unwrapped::Many(&self.errs)
    }
}

/// Go's `errors.Join(errs...)`: `None` when every error is nil, otherwise an error wrapping the non-nil ones
/// whose text is their texts joined by newlines.
pub fn join(errs: impl IntoIterator<Item = Option<Error>>) -> Option<Error> {
    let errs: Vec<Error> = errs.into_iter().flatten().collect();
    if errs.is_empty() {
        return None;
    }
    Some(Error::new(JoinError { errs }))
}

/// Whether `T` is the dynamic type of `err` (for callers that only need the check).
pub fn is_type<T: GoError>(err: &Error) -> bool {
    (err.as_dyn() as &dyn Any).type_id() == TypeId::of::<T>()
}
