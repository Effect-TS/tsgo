use std::ops::{BitAnd, BitOr, BitOrAssign, Not};

use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/effect_context.go:8 EffectContextFlags 5774b2b9c3b1c169
// Rust: a newtype over Go's `uint8` with the Go constants as associated consts and the bit operators Go uses.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct EffectContextFlags(pub u8);

// go: internal/typeparser/effect_context.go:10 EffectContextFlagNone 29bb874a57de5e83
impl EffectContextFlags {
    pub const None: EffectContextFlags = EffectContextFlags(0);
    // Go: `1 << iota` with iota 1 on this line.
    pub const CanYieldEffect: EffectContextFlags = EffectContextFlags(1 << 1);
    pub const InEffectConstructorThunk: EffectContextFlags = EffectContextFlags(1 << 2);
    pub const PendingNextFunctionIsEffectThunk: EffectContextFlags = EffectContextFlags(1 << 3);
    pub const PendingNextObjectTryPropertyIsEffectThunk: EffectContextFlags =
        EffectContextFlags(1 << 4);
    pub const InEffect: EffectContextFlags =
        EffectContextFlags(Self::CanYieldEffect.0 | Self::InEffectConstructorThunk.0);
}

impl BitOr for EffectContextFlags {
    type Output = EffectContextFlags;
    fn bitor(self, rhs: EffectContextFlags) -> EffectContextFlags {
        EffectContextFlags(self.0 | rhs.0)
    }
}

impl BitOrAssign for EffectContextFlags {
    fn bitor_assign(&mut self, rhs: EffectContextFlags) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for EffectContextFlags {
    type Output = EffectContextFlags;
    fn bitand(self, rhs: EffectContextFlags) -> EffectContextFlags {
        EffectContextFlags(self.0 & rhs.0)
    }
}

impl Not for EffectContextFlags {
    type Output = EffectContextFlags;
    fn not(self) -> EffectContextFlags {
        EffectContextFlags(!self.0)
    }
}

impl EffectContextFlags {
    /// Rust-only: Go's `flags&mask != 0`.
    pub fn intersects(self, mask: EffectContextFlags) -> bool {
        self.0 & mask.0 != 0
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_context.go:22 TypeParser.GetEffectContextFlags a8efb5b3f6b4ba7b
    // GetEffectContextFlags returns the Effect context of node. The analysis stores
    // an entry only at the source file and at nodes whose value differs from their
    // parent's, so a node answers with the entry of its closest ancestor.
    pub fn get_effect_context_flags(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> EffectContextFlags {
        let Some(node) = node.into() else {
            return EffectContextFlags::None;
        };
        let mut flags = get_closest_node_links(&self.links().effect_context_flags, node);
        if flags.is_none() && self.ensure_effect_context_analyzed(node) {
            flags = get_closest_node_links(&self.links().effect_context_flags, node);
        }
        let Some(flags) = flags else {
            return EffectContextFlags::None;
        };
        flags & EffectContextFlags::InEffect
    }

    // go: internal/typeparser/effect_context.go:38 TypeParser.GetEffectYieldGeneratorFunction 1c844de230f38018
    // GetEffectYieldGeneratorFunction returns the Effect.gen or Effect.fn generator
    // function node yields to, resolved like GetEffectContextFlags.
    pub fn get_effect_yield_generator_function(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<P<Node>> {
        let node = node.into()?;
        let mut gen_fn =
            get_closest_node_links(&self.links().effect_yield_generator_function, node);
        if gen_fn.is_none() && self.ensure_effect_context_analyzed(node) {
            gen_fn = get_closest_node_links(&self.links().effect_yield_generator_function, node);
        }
        gen_fn?
    }
}

// go: internal/typeparser/effect_context.go:55 getClosestNodeLinks b9265821502743b8
// getClosestNodeLinks returns the entry of node or of its closest ancestor
// with one. The source file always has an entry once analyzed, so nil means
// the file has not been analyzed yet.
//
// Rust: returns a copy of the entry (Go returns a pointer into the store, which its callers only read).
pub(crate) fn get_closest_node_links<T: Copy>(
    store: &DenseNodeStore<T>,
    node: P<Node>,
) -> Option<T> {
    let mut current = Some(node);
    while let Some(c) = current {
        if let Some(&links) = store.get(&c) {
            return Some(links);
        }
        current = c.parent();
    }
    None
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_context.go:66 TypeParser.ensureEffectContextAnalyzed d0932c57a1f51619
    // ensureEffectContextAnalyzed analyzes the source file of node once and reports
    // whether it has been analyzed.
    pub(crate) fn ensure_effect_context_analyzed(&mut self, node: P<Node>) -> bool {
        let Some(sf) = ets_shim::ast::get_source_file_of_node(node) else {
            return false;
        };
        cached(
            self,
            |l| &mut l.effect_context_analyzed,
            sf,
            |tp| {
                tp.analyze_effect_context_for_source_file(sf);
                true
            },
        )
    }
}

// go: internal/typeparser/effect_context.go:79 effectContextRegistration 254e17142c3fde9b
// effectContextRegistration is state a node registers for one of its
// descendants, applied when the walk reaches that descendant.
#[derive(Clone, Copy)]
pub(crate) struct EffectContextRegistration {
    node: P<Node>,
    enable_flags: EffectContextFlags,
    disable_flags: EffectContextFlags,
    generator_function: Option<P<Node>>,
    sets_generator: bool,
}

/// Rust-only: the state Go's `walk` closure in `analyzeEffectContextForSourceFile` captures.
struct EffectContextWalker<'a, 'c> {
    tp: &'a mut TypeParser<'c>,
    // registrations of the ancestors of the visited node, truncated after each subtree
    registrations: Vec<EffectContextRegistration>,
    // state of the parent of the visited node, and the flags it disables on its children
    parent_flags: EffectContextFlags,
    parent_generator_function: Option<P<Node>>,
    parent_children_disable_flags: EffectContextFlags,
    pending_flags_mask: EffectContextFlags,
    function_scope_reset_flags: EffectContextFlags,
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_context.go:87 TypeParser.analyzeEffectContextForSourceFile b19be467bd2a6cfc
    pub(crate) fn analyze_effect_context_for_source_file(&mut self, sf: P<SourceFile>) {
        let pending_flags_mask = EffectContextFlags::PendingNextFunctionIsEffectThunk
            | EffectContextFlags::PendingNextObjectTryPropertyIsEffectThunk;
        let function_scope_reset_flags = EffectContextFlags::CanYieldEffect
            | EffectContextFlags::InEffectConstructorThunk
            | pending_flags_mask;
        let mut walker = EffectContextWalker {
            tp: self,
            registrations: Vec::new(),
            parent_flags: EffectContextFlags::None,
            parent_generator_function: None,
            parent_children_disable_flags: EffectContextFlags::None,
            pending_flags_mask,
            function_scope_reset_flags,
        };
        walker.walk(sf.as_node());
    }
}

impl EffectContextWalker<'_, '_> {
    // go: internal/typeparser/effect_context.go:87 TypeParser.analyzeEffectContextForSourceFile b19be467bd2a6cfc
    // Rust: Go's `setPendingEnableFlags` closure.
    fn set_pending_enable_flags(&mut self, node: Option<P<Node>>, flags: EffectContextFlags) {
        let Some(node) = node else {
            return;
        };
        if flags == EffectContextFlags::None {
            return;
        }
        self.registrations.push(EffectContextRegistration {
            node,
            enable_flags: flags,
            disable_flags: EffectContextFlags::None,
            generator_function: None,
            sets_generator: false,
        });
    }

    // go: internal/typeparser/effect_context.go:87 TypeParser.analyzeEffectContextForSourceFile b19be467bd2a6cfc
    // Rust: Go's `setPendingDisableFlags` closure.
    fn set_pending_disable_flags(&mut self, node: Option<P<Node>>, flags: EffectContextFlags) {
        let Some(node) = node else {
            return;
        };
        if flags == EffectContextFlags::None {
            return;
        }
        self.registrations.push(EffectContextRegistration {
            node,
            enable_flags: EffectContextFlags::None,
            disable_flags: flags,
            generator_function: None,
            sets_generator: false,
        });
    }

    // go: internal/typeparser/effect_context.go:87 TypeParser.analyzeEffectContextForSourceFile b19be467bd2a6cfc
    // Rust: Go's `setPendingGeneratorFunction` closure. Go registers a nil node too (it never matches a visited
    // node); here a missing node registers nothing.
    fn set_pending_generator_function(&mut self, node: Option<P<Node>>, gen_fn: Option<P<Node>>) {
        let Some(node) = node else {
            return;
        };
        self.registrations.push(EffectContextRegistration {
            node,
            enable_flags: EffectContextFlags::None,
            disable_flags: EffectContextFlags::None,
            generator_function: gen_fn,
            sets_generator: true,
        });
    }

    // go: internal/typeparser/effect_context.go:87 TypeParser.analyzeEffectContextForSourceFile b19be467bd2a6cfc
    // Rust: Go's `walk` closure.
    fn walk(&mut self, node: P<Node>) -> bool {
        let registrations_len = self.registrations.len();

        // inherit from parent, if any; the source file starts with no flags.
        let mut flags = self.parent_flags;
        let mut generator_function = self.parent_generator_function;
        let mut disable_flags = self.parent_children_disable_flags;
        let mut enable_flags = EffectContextFlags::None;
        for registration in &self.registrations {
            if registration.node != node {
                continue;
            }
            disable_flags |= registration.disable_flags;
            enable_flags |= registration.enable_flags;
            if registration.sets_generator {
                generator_function = registration.generator_function;
            }
        }

        // disable pending disable flags, then merge pending state for this node
        flags = (flags & !disable_flags) | enable_flags;

        let pending_flags_mask = self.pending_flags_mask;
        let mut children_disable_flags = EffectContextFlags::None;
        if flags.intersects(EffectContextFlags::PendingNextFunctionIsEffectThunk)
            && (node.kind() == Kind::ArrowFunction || node.kind() == Kind::FunctionExpression)
        {
            if let Some(body) = node.body() {
                self.set_pending_enable_flags(
                    Some(body),
                    EffectContextFlags::InEffectConstructorThunk,
                );
                self.set_pending_disable_flags(Some(body), pending_flags_mask);
            }
        } else if flags.intersects(EffectContextFlags::PendingNextObjectTryPropertyIsEffectThunk)
            && node.kind() == Kind::ObjectLiteralExpression
        {
            children_disable_flags |= pending_flags_mask;
            let obj = node.as_object_literal_expression();
            for &prop in obj.properties().nodes() {
                if prop.kind() != Kind::PropertyAssignment {
                    continue;
                }
                // Rust: a property assignment always has a name and an initializer (Go's nil checks drop).
                let assignment = prop.as_property_assignment();
                if assignment.name().text() != "try" {
                    continue;
                }
                self.set_pending_enable_flags(
                    Some(assignment.initializer()),
                    EffectContextFlags::PendingNextFunctionIsEffectThunk,
                );
            }
        } else if let Some(expr) = transparent_pending_expression(node) {
            self.set_pending_enable_flags(Some(expr), flags & pending_flags_mask);
        } else if flags.intersects(pending_flags_mask) {
            children_disable_flags |= pending_flags_mask;
        }

        // logic for this node
        if let Some(effect_gen) = self.tp.effect_gen_call(node) {
            let body_node = effect_gen.body;
            self.set_pending_enable_flags(body_node, EffectContextFlags::CanYieldEffect);
            self.set_pending_generator_function(body_node, Some(effect_gen.generator_function));
        } else if let Some(effect_fn) = self.tp.effect_fn_call(node)
            && effect_fn.is_generator()
        {
            let body = effect_fn.body();
            let gen_fn = effect_fn.generator_function();
            if body.is_some() && gen_fn.is_some() {
                self.set_pending_enable_flags(body, EffectContextFlags::CanYieldEffect);
                self.set_pending_generator_function(body, gen_fn);
            }
        }

        if node.kind() == Kind::CallExpression {
            let call = node.as_call_expression();
            let arguments = call.arguments().nodes();
            if !arguments.is_empty() {
                let effect_thunk_arg = arguments[0];
                let expression = call.expression();
                if self.tp.is_node_reference_to_effect_module_api(expression, "sync")
                    || self.tp.is_node_reference_to_effect_module_api(expression, "promise")
                    || self.tp.is_node_reference_to_effect_module_api(expression, "callback")
                    || self.tp.is_node_reference_to_effect_module_api(expression, "suspend")
                {
                    self.set_pending_enable_flags(
                        Some(effect_thunk_arg),
                        EffectContextFlags::PendingNextFunctionIsEffectThunk,
                    );
                } else if self.tp.is_node_reference_to_effect_module_api(expression, "try")
                    || self.tp.is_node_reference_to_effect_module_api(expression, "tryPromise")
                {
                    self.set_pending_enable_flags(
                        Some(effect_thunk_arg),
                        EffectContextFlags::PendingNextFunctionIsEffectThunk
                            | EffectContextFlags::PendingNextObjectTryPropertyIsEffectThunk,
                    );
                }
            }
        }

        // Function-like nodes create a new scope, so they should not directly inherit
        // yieldability from an outer Effect scope. Matching Effect helpers re-enable the
        // flag on the specific body node below.
        if ets_shim::ast::is_function_like(Some(node)) {
            children_disable_flags |= self.function_scope_reset_flags;
        }

        // reset stores correlated to a flag set here.
        if !flags.intersects(EffectContextFlags::CanYieldEffect) {
            generator_function = None;
        }

        // store only what differs from the parent, reads resolve to the closest node with an entry.
        let is_source_file = node.parent().is_none();
        if is_source_file
            || flags & EffectContextFlags::InEffect
                != self.parent_flags & EffectContextFlags::InEffect
        {
            self.tp.links().effect_context_flags.insert(node, flags);
        }
        if is_source_file || generator_function != self.parent_generator_function {
            self.tp.links().effect_yield_generator_function.insert(node, generator_function);
        }

        let saved =
            (self.parent_flags, self.parent_generator_function, self.parent_children_disable_flags);
        self.parent_flags = flags;
        self.parent_generator_function = generator_function;
        self.parent_children_disable_flags = children_disable_flags;
        node.for_each_child(&mut |child| self.walk(child));
        (self.parent_flags, self.parent_generator_function, self.parent_children_disable_flags) =
            saved;
        self.registrations.truncate(registrations_len);
        false
    }
}

// go: internal/typeparser/effect_context.go:87 TypeParser.analyzeEffectContextForSourceFile b19be467bd2a6cfc
// Rust: Go's `transparentPendingExpression` closure (it captures nothing).
fn transparent_pending_expression(node: P<Node>) -> Option<P<Node>> {
    match node.kind() {
        Kind::ParenthesizedExpression
        | Kind::SatisfiesExpression
        | Kind::AsExpression
        | Kind::NonNullExpression
        | Kind::TypeAssertionExpression => node.expression(),
        _ => None,
    }
}
