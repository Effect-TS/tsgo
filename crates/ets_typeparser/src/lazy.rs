use std::sync::Arc;

use ets_shim::ast::Node;
use ets_shim::checker::Type;
use ets_shim::core::P;
use parking_lot::Mutex;

use crate::*;

/// Rust-only: a resolver of `lazy`. Go's closure captures the `*TypeParser` it was made by; here the parser is
/// passed in by `get`. Both are parsers over the same checker (the result is cached in that checker's links), so
/// they answer the same.
type Resolver<T> = Box<dyn FnOnce(&mut TypeParser<'_>) -> T + Send>;

// go: internal/typeparser/lazy.go:11 lazy 34e87cbe61ec3600
// lazy resolves a value on its first read and returns the stored value on
// later reads. Result structs hold it by pointer so their copies share one
// resolution. A checker runs on a single goroutine, so no lock is needed.
//
// Rust: Go's `*lazy[T]` is this shared handle (`Option<Lazy<T>>` where Go's pointer may be nil). The lock only
// makes the links `Send` (they live in a process-wide table); it is never contended.
pub struct Lazy<T>(Arc<Mutex<LazyState<T>>>);

struct LazyState<T> {
    resolve: Option<Resolver<T>>,
    value: T,
}

impl<T> Clone for Lazy<T> {
    fn clone(&self) -> Self {
        Lazy(Arc::clone(&self.0))
    }
}

// go: internal/typeparser/lazy.go:16 newLazy 398812cbba00a3c0
pub(crate) fn new_lazy<T: Default>(
    resolve: impl FnOnce(&mut TypeParser<'_>) -> T + Send + 'static,
) -> Lazy<T> {
    Lazy(Arc::new(Mutex::new(LazyState { resolve: Some(Box::new(resolve)), value: T::default() })))
}

// go: internal/typeparser/lazy.go:20 resolvedLazy 85ad53ef2648de9f
pub(crate) fn resolved_lazy<T>(value: T) -> Lazy<T> {
    Lazy(Arc::new(Mutex::new(LazyState { resolve: None, value })))
}

impl<T: Clone + Default> Lazy<T> {
    // go: internal/typeparser/lazy.go:24 lazy.get 2c815eb9982bd1c9
    // Rust: `this` is Go's receiver, which may be nil.
    pub(crate) fn get(this: Option<&Lazy<T>>, tp: &mut TypeParser<'_>) -> T {
        let Some(l) = this else {
            return T::default();
        };
        // The lock is not held while resolving: the resolver may read other lazies.
        let resolve = l.0.lock().resolve.take();
        if let Some(resolve) = resolve {
            let value = resolve(tp);
            l.0.lock().value = value;
        }
        l.0.lock().value.clone()
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/lazy.go:36 TypeParser.lazyTypeAtLocation 90da518a91e1a241
    #[expect(
        clippy::unused_self,
        reason = "a TypeParser method in Go; the lazy resolves through the parser it is read with"
    )]
    pub(crate) fn lazy_type_at_location(&self, node: P<Node>) -> Lazy<Option<P<Type>>> {
        new_lazy(move |tp| tp.get_type_at_location(node))
    }

    // go: internal/typeparser/lazy.go:40 TypeParser.lazyCallReturnType 954222397c36562f
    #[expect(
        clippy::unused_self,
        reason = "a TypeParser method in Go; the lazy resolves through the parser it is read with"
    )]
    pub(crate) fn lazy_call_return_type(&self, call: P<Node>) -> Lazy<Option<P<Type>>> {
        new_lazy(move |tp| {
            // Go: GetResolvedSignature never returns nil (unknownSignature when resolution fails).
            let signature = tp.checker.get_resolved_signature_exported(call);
            Some(tp.checker.get_return_type_of_signature_exported(signature))
        })
    }
}
