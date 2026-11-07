//! The ast-grep document type for GDScript.
//!
//! ast-grep compares source text only on leaf nodes. In tree-sitter-gdscript,
//! a string (`"Player"`), a string name (`&"x"`), a node path literal (`^"x"`)
//! and a node path (`$Player`, `%Health`) each have anonymous children (the
//! quotes, the `$`) but no node for their contents. With plain tree-sitter
//! nodes, ast-grep would see `"Player"` and `"Enemy"` as the same shape and
//! never compare their text, so `get_node("Player")` would match
//! `get_node("Enemy")`.
//!
//! [`GdNode`] wraps a tree-sitter node and reports those kinds as leaves with
//! no children, so ast-grep compares their full text. Every other method
//! delegates to tree-sitter.

use std::borrow::Cow;
use std::ops::Range;

use ast_grep_core::Position;
use ast_grep_core::source::{Doc, Edit, SgNode};
use ast_grep_core::tree_sitter::perform_edit;
use tree_sitter::{Node, Parser, Tree};

use super::language::GdScript;

/// Node kinds whose text is compared as a whole.
const OPAQUE_KINDS: [&str; 4] = ["string", "string_name", "node_path", "get_node"];

fn is_opaque(node: &Node) -> bool {
    OPAQUE_KINDS.contains(&node.kind())
}

fn parse(source: &str, old_tree: Option<&Tree>) -> Result<Tree, String> {
    let mut parser = Parser::new();
    if let Err(error) = parser.set_language(&tree_sitter_gdscript::LANGUAGE.into()) {
        return Err(error.to_string());
    }
    match parser.parse(source, old_tree) {
        Some(tree) => Ok(tree),
        None => Err("tree-sitter returned no tree".to_string()),
    }
}

/// A GDScript source and its tree, for ast-grep.
#[derive(Clone)]
pub struct GdDoc {
    source: String,
    tree: Tree,
}

impl GdDoc {
    /// Parses `source`. Used for patterns and tests.
    pub fn parse(source: &str) -> Result<GdDoc, String> {
        let tree = parse(source, None)?;
        Ok(GdDoc {
            source: source.to_string(),
            tree,
        })
    }

    /// Wraps an existing tree without parsing again. `tree` must be the tree
    /// of `source`.
    pub fn from_tree(source: &str, tree: &Tree) -> GdDoc {
        GdDoc {
            source: source.to_string(),
            tree: tree.clone(),
        }
    }
}

impl Doc for GdDoc {
    type Source = String;
    type Lang = GdScript;
    type Node<'r> = GdNode<'r>;

    fn get_lang(&self) -> &GdScript {
        &GdScript
    }

    fn get_source(&self) -> &String {
        &self.source
    }

    fn do_edit(&mut self, edit: &Edit<String>) -> Result<(), String> {
        perform_edit(&mut self.tree, &mut self.source, edit);
        self.tree = parse(&self.source, Some(&self.tree))?;
        Ok(())
    }

    fn root_node(&self) -> GdNode<'_> {
        GdNode(self.tree.root_node())
    }

    fn get_node_text<'a>(&'a self, node: &GdNode<'a>) -> Cow<'a, str> {
        Cow::Borrowed(&self.source[node.0.byte_range()])
    }
}

/// A tree-sitter node that hides the children of [`OPAQUE_KINDS`].
#[derive(Clone, Copy)]
pub struct GdNode<'r>(pub Node<'r>);

impl<'r> SgNode<'r> for GdNode<'r> {
    fn parent(&self) -> Option<Self> {
        let parent = self.0.parent()?;
        Some(GdNode(parent))
    }

    fn children(&self) -> impl ExactSizeIterator<Item = Self> {
        let count = if is_opaque(&self.0) {
            0
        } else {
            self.0.child_count()
        };
        let node = self.0;
        (0..count).map(move |index| match node.child(index) {
            Some(child) => GdNode(child),
            // `index < child_count`, so a child always exists.
            None => GdNode(node),
        })
    }

    fn kind(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.0.kind())
    }

    fn kind_id(&self) -> u16 {
        self.0.kind_id()
    }

    fn node_id(&self) -> usize {
        self.0.id()
    }

    fn range(&self) -> Range<usize> {
        self.0.byte_range()
    }

    fn start_pos(&self) -> Position {
        let point = self.0.start_position();
        Position::new(point.row, point.column, self.0.start_byte())
    }

    fn end_pos(&self) -> Position {
        let point = self.0.end_position();
        Position::new(point.row, point.column, self.0.end_byte())
    }

    fn next(&self) -> Option<Self> {
        let sibling = self.0.next_sibling()?;
        Some(GdNode(sibling))
    }

    fn prev(&self) -> Option<Self> {
        let sibling = self.0.prev_sibling()?;
        Some(GdNode(sibling))
    }

    fn is_named(&self) -> bool {
        self.0.is_named()
    }

    fn is_named_leaf(&self) -> bool {
        is_opaque(&self.0) || self.0.named_child_count() == 0
    }

    fn is_leaf(&self) -> bool {
        is_opaque(&self.0) || self.0.child_count() == 0
    }

    fn is_missing(&self) -> bool {
        self.0.is_missing()
    }

    fn is_error(&self) -> bool {
        self.0.is_error()
    }

    fn is_extra(&self) -> bool {
        self.0.is_extra()
    }

    fn field(&self, name: &str) -> Option<Self> {
        if is_opaque(&self.0) {
            return None;
        }
        let child = self.0.child_by_field_name(name)?;
        Some(GdNode(child))
    }

    fn field_children(&self, field_id: Option<u16>) -> impl Iterator<Item = Self> {
        let mut children = Vec::new();
        if let Some(field_id) = field_id
            && !is_opaque(&self.0)
        {
            let mut cursor = self.0.walk();
            if cursor.goto_first_child() {
                loop {
                    if let Some(current_field) = cursor.field_id()
                        && current_field.get() == field_id
                    {
                        children.push(GdNode(cursor.node()));
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }
        children.into_iter()
    }

    fn child_by_field_id(&self, field_id: u16) -> Option<Self> {
        if is_opaque(&self.0) {
            return None;
        }
        let child = self.0.child_by_field_id(field_id)?;
        Some(GdNode(child))
    }
}
