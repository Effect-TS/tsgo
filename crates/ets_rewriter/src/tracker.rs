use ets_shim::ast::{Kind, ModifierList, Node, NodeFactory, NodeFlags, SourceFile};
use ets_shim::core::P;
use ets_shim::core::TextRange;
use ets_shim::core::alloc_str;
use ets_shim::core::collections::OrderedMap;
use ets_shim::ls::change;
use ets_shim::ls::format;
use ets_shim::lsproto;
use ets_shim::scanner;

// go: internal/rewriter/tracker.go:14 NodeOptions e8d7eccb76d596cb
pub type NodeOptions = change::NodeOptions;
// go: internal/rewriter/tracker.go:15 LeadingTriviaOption 0a8a032cb8ce61c9
pub type LeadingTriviaOption = change::LeadingTriviaOption;
// go: internal/rewriter/tracker.go:16 TrailingTriviaOption c6053cf278b29c0a
pub type TrailingTriviaOption = change::TrailingTriviaOption;

// go: internal/rewriter/tracker.go:18 LeadingTriviaOptionNone f16ee4fc3a7ab318
pub const LEADING_TRIVIA_OPTION_NONE: LeadingTriviaOption = change::LeadingTriviaOption::None;
// go: internal/rewriter/tracker.go:19 LeadingTriviaOptionExclude 48f281e3e6040d7e
pub const LEADING_TRIVIA_OPTION_EXCLUDE: LeadingTriviaOption = change::LeadingTriviaOption::Exclude;
// go: internal/rewriter/tracker.go:20 LeadingTriviaOptionIncludeAll 32ac46105e41c814
pub const LEADING_TRIVIA_OPTION_INCLUDE_ALL: LeadingTriviaOption =
    change::LeadingTriviaOption::IncludeAll;
// go: internal/rewriter/tracker.go:21 LeadingTriviaOptionJSDoc af9c5a3794c67c36
pub const LEADING_TRIVIA_OPTION_JS_DOC: LeadingTriviaOption = change::LeadingTriviaOption::JSDoc;
// go: internal/rewriter/tracker.go:22 LeadingTriviaOptionStartLine 44ce9d93dd1b8a29
pub const LEADING_TRIVIA_OPTION_START_LINE: LeadingTriviaOption =
    change::LeadingTriviaOption::StartLine;

// go: internal/rewriter/tracker.go:24 TrailingTriviaOptionNone 5a4d0ea22eeea528
pub const TRAILING_TRIVIA_OPTION_NONE: TrailingTriviaOption = change::TrailingTriviaOption::None;
// go: internal/rewriter/tracker.go:25 TrailingTriviaOptionExclude 53999591ad3102bc
pub const TRAILING_TRIVIA_OPTION_EXCLUDE: TrailingTriviaOption =
    change::TrailingTriviaOption::Exclude;
// go: internal/rewriter/tracker.go:26 TrailingTriviaOptionExcludeWhitespace 06cd00c71c0c37e7
pub const TRAILING_TRIVIA_OPTION_EXCLUDE_WHITESPACE: TrailingTriviaOption =
    change::TrailingTriviaOption::ExcludeWhitespace;
// go: internal/rewriter/tracker.go:27 TrailingTriviaOptionInclude 2578eed371f5b74e
pub const TRAILING_TRIVIA_OPTION_INCLUDE: TrailingTriviaOption =
    change::TrailingTriviaOption::Include;

// go: internal/rewriter/tracker.go:29 pendingBeforeEdit b21b484a65a2b83b
// Rust: Go's nil `sourceFile` / `before` cannot be stored (InsertNodeBefore returns early on nil), so the
// fields are plain handles.
#[derive(Clone)]
pub(crate) struct PendingBeforeEdit {
    pub(crate) source_file: P<SourceFile>,
    pub(crate) before: P<Node>,
    pub(crate) text: String,
    pub(crate) blank_line_between: bool,
    pub(crate) leading: change::LeadingTriviaOption,
}

// go: internal/rewriter/tracker.go:37 Tracker c79a6fb6b9df7ffc
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

// go: internal/rewriter/tracker.go:42 NewTracker 6d5a09bb9eef0e43
pub fn new_tracker(raw: change::Tracker) -> Tracker {
    Tracker { tracker: raw, pending_before: Vec::new() }
}

impl Tracker {
    // go: internal/rewriter/tracker.go:46 Tracker.GetChanges 120f5f95830f6621
    // Rust: Go's shim `change.CollectChanges` keeps the edits of `GetChanges` and drops the unmappable file list.
    pub fn get_changes(&mut self) -> OrderedMap<String, Vec<lsproto::TextEdit>> {
        self.flush_pending_before();
        self.tracker.get_changes().0
    }

    // go: internal/rewriter/tracker.go:51 Tracker.NewModuleDeclaration d4d8e550308dd841
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

    // go: internal/rewriter/tracker.go:84 Tracker.ReplaceNode 88acc49e3960f376
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
        // Rust: Go takes the start from the shim's `change.Tracker_getAdjustedStartPosition` and the range again in
        // `change.ReplaceAdjustedRangeWithText` (the legacy provider's range is in LSP positions); tsrs's
        // `get_adjusted_range` is in source positions, so the Rust computes it once and uses its start for both.
        let rng = self.tracker.get_adjusted_range(
            source_file,
            old_node,
            old_node,
            options.leading_trivia_option,
            options.trailing_trivia_option,
        );
        let mut text = self.consume_pending_prefix(
            source_file,
            Some(old_node),
            rng.pos().expect_source("an adjusted range in the file"),
        ) + &self.node_text(source_file, new_node);
        if !options.prefix.is_empty() {
            text = options.prefix.clone() + &text;
        }
        if !options.suffix.is_empty() {
            text += &options.suffix;
        }
        // Go's shim `change.ReplaceAdjustedRangeWithText`.
        self.tracker.replace_text_range_with_text(source_file, rng, &text);
    }

    // go: internal/rewriter/tracker.go:116 Tracker.InsertNodeBefore e6e91629be5f17cd
    // InsertNodeBefore inserts newNode as a statement before the statement
    // before. The text is laid out when the edits are collected, so newNode takes
    // the indentation of before's line, and a replacement of before keeps it.
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
        let text = self.node_text(source_file, new_node);
        self.pending_before.push(PendingBeforeEdit {
            source_file,
            before,
            text,
            blank_line_between,
            leading: leading_trivia_option,
        });
    }

    // go: internal/rewriter/tracker.go:129 Tracker.flushPendingBefore b487f7e845117015
    pub(crate) fn flush_pending_before(&mut self) {
        for edit in std::mem::take(&mut self.pending_before) {
            if edit.text.is_empty() {
                continue;
            }
            // Rust: Go's `change.Tracker_getAdjustedStartPosition` plus the shim's
            // `change.InsertTextAtAdjustedRangeStart` is the start of tsrs's `get_adjusted_range`.
            let start = self
                .tracker
                .get_adjusted_range(
                    edit.source_file,
                    edit.before,
                    edit.before,
                    edit.leading,
                    change::TrailingTriviaOption::None,
                )
                .pos();
            let text =
                pending_before_text(&edit, start.expect_source("an adjusted range in the file"));
            self.tracker.replace_text_range_with_text(
                edit.source_file,
                TextRange::from_text_pos(start, start),
                &text,
            );
        }
        self.pending_before = Vec::new();
    }

    // go: internal/rewriter/tracker.go:171 Tracker.consumePendingPrefix a1ebe4d91919b7dd
    // consumePendingPrefix takes the pending inserts before target, laid out for a
    // replacement of target that starts at pos.
    pub(crate) fn consume_pending_prefix(
        &mut self,
        source_file: P<SourceFile>,
        target: Option<P<Node>>,
        pos: u32,
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
                builder.push_str(&pending_before_text(&edit, pos));
                continue;
            }
            kept.push(edit);
        }
        self.pending_before = kept;
        builder
    }

    // go: internal/rewriter/tracker.go:188 Tracker.nodeText 35e9e4b510af8fd8
    pub(crate) fn node_text(&self, source_file: P<SourceFile>, node: P<Node>) -> String {
        let (text, _) = self.tracker.get_nonformatted_text(node, source_file);
        text
    }
}

// go: internal/rewriter/tracker.go:152 pendingBeforeText 22a02f104b0a54cd
// pendingBeforeText lays out the text of edit inserted at pos. When before
// starts its line, the text gets the line's indentation and before moves to the
// next line, indented as it was; when code precedes before on its line, as in
// `{ f() }`, the text is joined with a space.
pub(crate) fn pending_before_text(edit: &PendingBeforeEdit, pos: u32) -> String {
    let source_text = edit.source_file.text();
    let before_start = scanner::get_token_pos_of_node(edit.before, edit.source_file, false);
    let line_start = format::get_line_start_position_for_position(before_start, edit.source_file);
    let indentation = &source_text[line_start as usize..before_start as usize];
    if !indentation.trim_start_matches([' ', '\t']).is_empty() {
        return edit.text.clone() + " ";
    }
    let new_lines = if edit.blank_line_between { "\n\n" } else { "\n" };
    if pos == format::get_line_start_position_for_position(pos, edit.source_file) {
        return indentation.to_string() + &edit.text + new_lines;
    }
    edit.text.clone() + new_lines + indentation
}

/// Rust-only: the path builders of `Tracker` only build nodes, so they are ported over the node factory, which
/// helpers that take `&NodeFactory` (instead of the tracker) reach too.
pub trait NodeFactoryPaths {
    fn new_property_access_path<S: AsRef<str>>(&self, names: &[S]) -> P<Node>;
    fn new_qualified_name_path<S: AsRef<str>>(&self, names: &[S]) -> P<Node>;
}

impl NodeFactoryPaths for NodeFactory {
    // go: internal/rewriter/tracker.go:61 Tracker.NewPropertyAccessPath 494eb42f358a1fc5
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

    // go: internal/rewriter/tracker.go:73 Tracker.NewQualifiedNamePath b072f8d166f7910b
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
