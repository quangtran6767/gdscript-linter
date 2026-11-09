/** Frozen SM0 contract for sandboxed .js rules; .ts files are not executed. */
declare module "gdguard" {
  /** A source position: line/column are 1-based UTF-8 bytes; byte is 0-based. */
  export interface Position {
    /** One-based line. */
    readonly line: number;
    /** One-based UTF-8 byte column. */
    readonly column: number;
    /** Absolute UTF-8 byte offset, not a JavaScript string index. */
    readonly byte: number;
  }

  /** Half-open UTF-8 source range. */
  export interface Range {
    /** Inclusive start. */
    readonly start: Position;
    /** Exclusive end. */
    readonly end: Position;
  }

  /** Lazily wrapped tree-sitter node, valid only during the current file check. */
  export interface Node {
    /** Grammar kind, such as function_definition or identifier. */
    readonly type: string;
    /** Exact source text of this node. */
    readonly text: string;
    /** Positions in the original source. */
    readonly range: Range;
    /** Enclosing node, or null at the root. */
    readonly parent: Node | null;
    /** All children, including punctuation. */
    readonly children: readonly Node[];
    /** Grammar-named children only. */
    readonly namedChildren: readonly Node[];
    /** Named grammar field, or null when absent. */
    field(name: string): Node | null;
  }

  /** One replacement, applied atomically with the other edits in its fix. */
  export interface Edit {
    /** Half-open range to replace. Insertions have equal start/end. */
    readonly range: Range;
    /** Replacement text (empty to remove). */
    readonly replacement: string;
  }

  /** Edit constructors; they never mutate source directly. */
  export interface Fixer {
    /** Replace the complete node/range. */
    replace(target: Node | Range, text: string): Edit;
    /** Insert at the target's start byte. */
    insertBefore(target: Node | Range, text: string): Edit;
    /** Insert at the target's exclusive end byte. */
    insertAfter(target: Node | Range, text: string): Edit;
    /** Remove the complete node/range. */
    remove(target: Node | Range): Edit;
  }

  /** Fix safety: suggestions are never automatically applied. */
  export type FixSafety = "safe" | "unsafe" | "suggestion";
  /** Severity before config overrides. */
  export type Severity = "error" | "warning" | "info";
  /** Reported fix, or a callback evaluated with the file's fixer. */
  export type Fix = Edit | readonly Edit[] | ((fixer: Fixer) => Edit | readonly Edit[] | null);
  /** A diagnostic with exactly one location, in the current file. */
  export type ReportDescriptor = {
    /** One sentence describing the problem. */
    message: string;
    /** Optional actionable guidance. */
    help?: string;
    /** Optional edits or callback producing non-overlapping edits. */
    fix?: Fix;
    /** Override meta.fixSafety for this diagnostic. Default: suggestion. */
    fixSafety?: FixSafety;
    /** Optional short description of the proposed fix. */
    fixDescription?: string;
  } & (
    { /** Report on this node. */ node: Node; range?: never }
    | { node?: never; /** Report on an explicit byte range. */ range: Range }
  );

  /** Frozen lexical symbol kinds. */
  export type SymbolKind = "variable" | "constant" | "parameter" | "loop_variable"
    | "function" | "signal" | "enum" | "enum_member" | "inner_class";
  /** A declaration in this file; types are annotations, never inferred. */
  export interface Symbol {
    /** File-local symbol ID. */
    readonly id: number;
    /** Declared name. */
    readonly name: string;
    /** Declaration kind. */
    readonly kind: SymbolKind;
    /** File-local declaring scope ID. */
    readonly scope: number;
    /** Identifier range. */
    readonly nameSpan: Range;
    /** Full declaration range. */
    readonly declarationSpan: Range;
    /** Explicit annotation text, or null. */
    readonly declaredType: string | null;
    /** Annotation names without @, such as onready and export. */
    readonly annotations: readonly string[];
    /** Whether declared with static. */
    readonly isStatic: boolean;
  }

  /** An identifier use; unresolved names may be inherited or engine names. */
  export interface Reference {
    /** Identifier location. */
    readonly range: Range;
    /** Identifier text. */
    readonly name: string;
    /** Use classification. A compound assignment has read and write uses. */
    readonly kind: "read" | "write" | "call";
    /** Resolved file-local ID, or null. */
    readonly symbol: number | null;
  }

  /** Read-only lexical model, built once for this file. */
  export interface Semantic {
    /** Declarations in ID order. */
    readonly symbols: readonly Symbol[];
    /** Innermost scope at a UTF-8 byte offset, or root (0) outside the file. */
    scopeAt(byte: number): number;
    /** Lexical lookup, returning a file-local symbol ID or null. */
    lookup(scope: number, name: string): number | null;
    /** Uses of a declaration; unknown IDs return an empty array. */
    referencesTo(symbol: number): readonly Reference[];
  }

  /** Read-only project data. No engine class database is assumed. */
  export interface Project {
    /** Sorted names declared with class_name. */
    readonly classNames: readonly string[];
    /** Autoload name to script path, relative to the project root. */
    readonly autoloads: Readonly<Record<string, string>>;
    /** Follows project inheritance; unknown when an engine base is reached. */
    hasMember(className: string, member: string): "yes" | "no" | "unknown";
  }

  /** File context supplied once to create(). */
  export interface RuleContext<Options = Record<string, unknown>> {
    /** Path used by config and reports. */
    readonly filename: string;
    /** Entire original source. String indices use UTF-16; ranges use UTF-8. */
    readonly source: string;
    /** Effective rule options after per-folder overrides. */
    readonly options: Readonly<Options>;
    /** Record a diagnostic for this file. */
    report(descriptor: ReportDescriptor): void;
    /** Helpers for constructing edits. */
    readonly fix: Fixer;
    /** Lexical model; null only when the caller did not supply one. */
    readonly semantic: Semantic | null;
    /** Project model, or null outside a Godot project. */
    readonly project: Project | null;
  }

  /** Metadata shown by explain/list-rules and used for diagnostic defaults. */
  export interface RuleMeta {
    /** Unique kebab-case ID across built-in, YAML and JS rules. */
    id: string;
    /** Default warning when omitted. */
    severity?: Severity;
    /** One-line catalog description. */
    description: string;
    /** Why this rule exists; shown by explain. */
    rationale?: string;
    /** Whether safe or unsafe automatic fixes may be offered. */
    fixable?: boolean;
    /** Default for fixes reported by this rule; suggestion when omitted. */
    fixSafety?: FixSafety;
  }

  /** Embedded rule tests use only this rule, independent of built-ins. */
  export interface RuleTests {
    /** Snippets producing no diagnostics. */
    valid?: readonly string[];
    /** Snippets producing diagnostics; output also checks parsing/idempotency. */
    invalid?: readonly (string | {
      /** Input source. */
      code: string;
      /** Exact diagnostic count; at least one when omitted. */
      errors?: number;
      /** Expected source after applying this rule's fixes at any safety. */
      output?: string;
    })[];
  }

  /** Node-kind visitor map. A rule is called only for subscribed kinds. */
  export type Visitors = Record<string, (node: Node) => void>;
  /** Default ES module export. No import, require, network or filesystem API. */
  export interface Rule<Options = Record<string, unknown>> {
    /** Validated metadata. */
    meta: RuleMeta;
    /** Called once per file; return handlers by exact grammar node kind. */
    create(context: RuleContext<Options>): Visitors;
    /** Cases run by gdguard test-rules. */
    tests?: RuleTests;
  }
}
