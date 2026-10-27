use ets_shim::ast::{Kind, ModifierList, Node, NodeFactory, NodeFlags, SourceFile};
use ets_shim::core::P;
use ets_shim::core::TextRange;
use ets_shim::core::alloc_str;
use ets_shim::core::collections::OrderedMap;
use ets_shim::ls::change;
use ets_shim::lsproto;

// go: internal/rewriter/tracker.go:12 NodeOptions
pub type NodeOptions = change::NodeOptions;
// go: internal/rewriter/tracker.go:13 LeadingTriviaOption
pub type LeadingTriviaOption = change::LeadingTriviaOption;
// go: internal/rewriter/tracker.go:14 TrailingTriviaOption
pub type TrailingTriviaOption = change::TrailingTriviaOption;

// go: internal/rewriter/tracker.go:16 LeadingTriviaOptionNone
pub const LEADING_TRIVIA_OPTION_NONE: LeadingTriviaOption = change::LeadingTriviaOption::None;
// go: internal/rewriter/tracker.go:17 LeadingTriviaOptionExclude
pub const LEADING_TRIVIA_OPTION_EXCLUDE: LeadingTriviaOption = change::LeadingTriviaOption::Exclude;
// go: internal/rewriter/tracker.go:18 LeadingTriviaOptionIncludeAll
pub const LEADING_TRIVIA_OPTION_INCLUDE_ALL: LeadingTriviaOption =
    change::LeadingTriviaOption::IncludeAll;
// go: internal/rewriter/tracker.go:19 LeadingTriviaOptionJSDoc
pub const LEADING_TRIVIA_OPTION_JS_DOC: LeadingTriviaOption = change::LeadingTriviaOption::JSDoc;
// go: internal/rewriter/tracker.go:20 LeadingTriviaOptionStartLine
pub const LEADING_TRIVIA_OPTION_START_LINE: LeadingTriviaOption =
    change::LeadingTriviaOption::StartLine;

// go: internal/rewriter/tracker.go:22 TrailingTriviaOptionNone
pub const TRAILING_TRIVIA_OPTION_NONE: TrailingTriviaOption = change::TrailingTriviaOption::None;
// go: internal/rewriter/tracker.go:23 TrailingTriviaOptionExclude
pub const TRAILING_TRIVIA_OPTION_EXCLUDE: TrailingTriviaOption =
    change::TrailingTriviaOption::Exclude;
// go: internal/rewriter/tracker.go:24 TrailingTriviaOptionExcludeWhitespace
pub const TRAILING_TRIVIA_OPTION_EXCLUDE_WHITESPACE: TrailingTriviaOption =
    change::TrailingTriviaOption::ExcludeWhitespace;
// go: internal/rewriter/tracker.go:25 TrailingTriviaOptionInclude
pub const TRAILING_TRIVIA_OPTION_INCLUDE: TrailingTriviaOption =
    change::TrailingTriviaOption::Include;

// go: internal/rewriter/tracker.go:27 pendingBeforeEdit
// Rust: Go's nil `sourceFile` / `before` cannot be stored (InsertNodeBefore returns early on nil), so the
// fields are plain handles.
#[derive(Clone)]
pub(crate) struct PendingBeforeEdit {
    pub(crate) source_file: P<SourceFile>,
    pub(crate) before: P<Node>,
    pub(crate) text: String,
    pub(crate) leading: change::LeadingTriviaOption,
}

// go: internal/rewriter/tracker.go:34 Tracker
// Rust: Go embeds `*change.Tracker`, promoting all of its methods (and its embedded `NodeFactory`'s). The
// embedding is a named field plus `Deref`/`DerefMut` to it, which reproduces Go's promotion: the methods defined
// here (`replace_node`, `insert_node_before`, `get_changes`, ...) shadow the promoted ones of the same name, as
// in Go, and every other `change::Tracker` / `NodeFactory` method is reachable as `t.method(..)`.
pub struct Tracker {
    pub tracker: change::Tracker,
    pub(crate) pending_before: Vec<PendingBeforeEdit>,
}

impl std::ops::Deref for Tracker {
    type Target = change::Tracker;
    fn deref(&self) -> &change::Tracker {
        &self.tracker
    }
}

impl std::ops::DerefMut for Tracker {
    fn deref_mut(&mut self) -> &mut change::Tracker {
        &mut self.tracker
    }
}

// go: internal/rewriter/tracker.go:39 NewTracker
pub fn new_tracker(raw: change::Tracker) -> Tracker {
    Tracker { tracker: raw, pending_before: Vec::new() }
}

impl Tracker {
    // go: internal/rewriter/tracker.go:43 Tracker.GetChanges
    // Rust: Go's shim `change.CollectChanges` keeps the edits of `GetChanges` and drops the unmappable file list.
    pub fn get_changes(&mut self) -> OrderedMap<String, Vec<lsproto::TextEdit>> {
        self.flush_pending_before();
        self.tracker.get_changes().0
    }

    // go: internal/rewriter/tracker.go:48 Tracker.NewModuleDeclaration
    // Rust: Go's shim `change.NewModuleDeclaration` calls the factory with nil attributes.
    pub fn new_module_declaration(
        &self,
        modifiers: Option<P<ModifierList>>,
        keyword: Kind,
        name: P<Node>,
        body: Option<P<Node>>,
    ) -> P<Node> {
        self.tracker.new_module_declaration(modifiers, keyword, name, None, body)
    }

    /// Rust-only: `Tracker.NewPropertyAccessPath` (ported on `NodeFactoryPaths`), callable without importing the
    /// trait.
    pub fn new_property_access_path<S: AsRef<str>>(&self, names: &[S]) -> P<Node> {
        NodeFactoryPaths::new_property_access_path(&*self.tracker, names)
    }

    /// Rust-only: `Tracker.NewQualifiedNamePath` (ported on `NodeFactoryPaths`), callable without importing the
    /// trait.
    pub fn new_qualified_name_path<S: AsRef<str>>(&self, names: &[S]) -> P<Node> {
        NodeFactoryPaths::new_qualified_name_path(&*self.tracker, names)
    }

    // go: internal/rewriter/tracker.go:81 Tracker.ReplaceNode
    pub fn replace_node(
        &mut self,
        source_file: P<SourceFile>,
        old_node: Option<P<Node>>,
        new_node: Option<P<Node>>,
        options: Option<&NodeOptions>,
    ) {
        let (Some(old_node), Some(new_node)) = (old_node, new_node) else {
            return;
        };
        let default_options;
        let options = if let Some(options) = options {
            options
        } else {
            let mut o = change::NodeOptions::default();
            o.leading_trivia_option = change::LeadingTriviaOption::Exclude;
            o.trailing_trivia_option = change::TrailingTriviaOption::Exclude;
            default_options = o;
            &default_options
        };
        let mut text = self.consume_pending_prefix(source_file, Some(old_node))
            + &self.node_text(source_file, new_node);
        if !options.prefix.is_empty() {
            text = options.prefix.clone() + &text;
        }
        if !options.suffix.is_empty() {
            text += &options.suffix;
        }
        // Go's shim `change.ReplaceAdjustedRangeWithText`.
        let rng = self.tracker.get_adjusted_range(
            source_file,
            old_node,
            old_node,
            options.leading_trivia_option,
            options.trailing_trivia_option,
        );
        self.tracker.replace_text_range_with_text(source_file, rng, &text);
    }

    // go: internal/rewriter/tracker.go:109 Tracker.InsertNodeBefore
    pub fn insert_node_before(
        &mut self,
        source_file: P<SourceFile>,
        before: Option<P<Node>>,
        new_node: Option<P<Node>>,
        blank_line_between: bool,
        leading_trivia_option: change::LeadingTriviaOption,
    ) {
        let (Some(before), Some(new_node)) = (before, new_node) else {
            return;
        };
        let mut text = self.node_text(source_file, new_node);
        if blank_line_between {
            text += "\n\n";
        } else {
            text += "\n";
        }
        self.pending_before.push(PendingBeforeEdit {
            source_file,
            before,
            text,
            leading: leading_trivia_option,
        });
    }

    // go: internal/rewriter/tracker.go:122 Tracker.flushPendingBefore
    pub(crate) fn flush_pending_before(&mut self) {
        for edit in std::mem::take(&mut self.pending_before) {
            if edit.text.is_empty() {
                continue;
            }
            // Go's shim `change.InsertTextAtAdjustedRangeStart`.
            let rng = self.tracker.get_adjusted_range(
                edit.source_file,
                edit.before,
                edit.before,
                edit.leading,
                change::TrailingTriviaOption::None,
            );
            let start = rng.pos();
            self.tracker.replace_text_range_with_text(
                edit.source_file,
                TextRange::from_text_pos(start, start),
                &edit.text,
            );
        }
        self.pending_before = Vec::new();
    }

    // go: internal/rewriter/tracker.go:140 Tracker.consumePendingPrefix
    pub(crate) fn consume_pending_prefix(
        &mut self,
        source_file: P<SourceFile>,
        target: Option<P<Node>>,
    ) -> String {
        let Some(target) = target else {
            return String::new();
        };
        if self.pending_before.is_empty() {
            return String::new();
        }
        let mut builder = String::new();
        let mut kept = Vec::with_capacity(self.pending_before.len());
        for edit in std::mem::take(&mut self.pending_before) {
            if edit.source_file == source_file && edit.before == target {
                builder.push_str(&edit.text);
                continue;
            }
            kept.push(edit);
        }
        self.pending_before = kept;
        builder
    }

    // go: internal/rewriter/tracker.go:157 Tracker.nodeText
    pub(crate) fn node_text(&self, source_file: P<SourceFile>, node: P<Node>) -> String {
        let (text, _) = self.tracker.get_nonformatted_text(node, source_file);
        text
    }
}

/// Rust-only: the path builders of `Tracker` only build nodes, so they are ported over the node factory, which
/// helpers that take `&NodeFactory` (instead of the tracker) reach too.
pub trait NodeFactoryPaths {
    fn new_property_access_path<S: AsRef<str>>(&self, names: &[S]) -> P<Node>;
    fn new_qualified_name_path<S: AsRef<str>>(&self, names: &[S]) -> P<Node>;
}

impl NodeFactoryPaths for NodeFactory {
    // go: internal/rewriter/tracker.go:58 Tracker.NewPropertyAccessPath
    // NewPropertyAccessPath builds the expression a.b.c from ["a", "b", "c"].
    //
    // Rust: Go returns nil for an empty path; every caller passes a module reference, which is never empty, so the
    // Rust returns the node and panics on an empty path instead of making every caller unwrap.
    fn new_property_access_path<S: AsRef<str>>(&self, names: &[S]) -> P<Node> {
        assert!(!names.is_empty(), "NewPropertyAccessPath of an empty path");
        let mut node = self.new_identifier(alloc_str(names[0].as_ref()));
        for name in &names[1..] {
            node = self.new_property_access_expression(
                node,
                None,
                self.new_identifier(alloc_str(name.as_ref())),
                NodeFlags::None,
            );
        }
        node
    }

    // go: internal/rewriter/tracker.go:70 Tracker.NewQualifiedNamePath
    // NewQualifiedNamePath builds the type-position name a.b.c from ["a", "b", "c"].
    //
    // Rust: as NewPropertyAccessPath, an empty path panics instead of returning nil.
    fn new_qualified_name_path<S: AsRef<str>>(&self, names: &[S]) -> P<Node> {
        assert!(!names.is_empty(), "NewQualifiedNamePath of an empty path");
        let mut node = self.new_identifier(alloc_str(names[0].as_ref()));
        for name in &names[1..] {
            node = self.new_qualified_name(node, self.new_identifier(alloc_str(name.as_ref())));
        }
        node
    }
}
