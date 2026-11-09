//! Frozen SM0 semantic contracts. W5 fills the builder; unresolved names
//! must remain data rather than diagnostics (they may come from the engine).

mod builder;

use crate::linter::diagnostic::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScopeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Class,
    InnerClass,
    Function,
    Lambda,
    Block,
    ForLoop,
    MatchBranch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub span: Span,
    /// Declaration order within this scope.
    pub symbols: Vec<SymbolId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Constant,
    Parameter,
    LoopVariable,
    Function,
    Signal,
    Enum,
    EnumMember,
    InnerClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub scope: ScopeId,
    pub name_span: Span,
    pub declaration_span: Span,
    pub declared_type: Option<String>,
    pub annotations: Vec<String>,
    pub is_static: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReferenceKind {
    Read,
    Write,
    Call,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub span: Span,
    pub name: String,
    pub kind: ReferenceKind,
    pub symbol: Option<SymbolId>,
}

#[derive(Debug, Clone)]
pub struct SemanticModel {
    scopes: Vec<Scope>,
    symbols: Vec<Symbol>,
    references: Vec<Vec<Reference>>,
    unresolved: Vec<Reference>,
}

impl SemanticModel {
    /// SM0 supplies a valid root scope; W5 implements declaration/resolution.
    pub fn build(_tree: &tree_sitter::Tree, source: &str) -> SemanticModel {
        SemanticModel {
            scopes: vec![Scope {
                id: ScopeId(0),
                kind: ScopeKind::Class,
                parent: None,
                span: Span::from_byte_range(source, 0, source.len()),
                symbols: Vec::new(),
            }],
            symbols: Vec::new(),
            references: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    /// Returns the innermost scope, or root scope 0 outside the source.
    pub fn scope_at(&self, _byte: usize) -> ScopeId {
        ScopeId(0)
    }

    pub fn lookup(&self, _scope: ScopeId, _name: &str) -> Option<SymbolId> {
        None
    }

    /// Invalid IDs return None rather than panicking.
    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.0 as usize)
    }

    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    pub fn references_to(&self, id: SymbolId) -> &[Reference] {
        match self.references.get(id.0 as usize) {
            Some(references) => references,
            None => &[],
        }
    }

    pub fn unresolved_references(&self) -> &[Reference] {
        &self.unresolved
    }

    /// Stable UTF-8 text, ending each entry with LF. Scopes and symbols are
    /// in ID order. References follow in (start byte, end byte, kind) order.
    /// Strings/optional strings/lists use Rust Debug escaping; spans are
    /// half-open UTF-8 bytes; missing parent/resolution is `-`.
    ///
    /// ```text
    /// scope 0 Class 0..0 parent=-
    /// symbol 0 Variable "x" scope=0 name=4..5 decl=0..9 type=None annotations=[] static=false
    /// ref Read "x" 10..11 symbol=0
    /// ```
    pub fn debug_dump(&self) -> String {
        let mut output = String::new();
        for scope in &self.scopes {
            let parent = match scope.parent {
                Some(parent) => parent.0.to_string(),
                None => "-".to_string(),
            };
            output.push_str(&format!(
                "scope {} {:?} {}..{} parent={}\n",
                scope.id.0, scope.kind, scope.span.start_byte, scope.span.end_byte, parent
            ));
        }
        for symbol in &self.symbols {
            output.push_str(&format!(
                "symbol {} {:?} {:?} scope={} name={}..{} decl={}..{} type={:?} annotations={:?} static={}\n",
                symbol.id.0, symbol.kind, symbol.name, symbol.scope.0,
                symbol.name_span.start_byte, symbol.name_span.end_byte,
                symbol.declaration_span.start_byte, symbol.declaration_span.end_byte,
                symbol.declared_type, symbol.annotations, symbol.is_static
            ));
        }
        let mut reference_count = self.unresolved.len();
        for symbol_references in &self.references {
            reference_count += symbol_references.len();
        }
        let mut references = Vec::with_capacity(reference_count);
        for symbol_references in &self.references {
            references.extend(symbol_references);
        }
        references.extend(&self.unresolved);
        references.sort_by_key(|reference| {
            (
                reference.span.start_byte,
                reference.span.end_byte,
                reference.kind,
            )
        });
        for reference in references {
            let symbol = match reference.symbol {
                Some(symbol) => symbol.0.to_string(),
                None => "-".to_string(),
            };
            output.push_str(&format!(
                "ref {:?} {:?} {}..{} symbol={}\n",
                reference.kind,
                reference.name,
                reference.span.start_byte,
                reference.span.end_byte,
                symbol
            ));
        }
        output
    }
}
