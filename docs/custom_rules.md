# Custom rules

gdguard runs your own rules next to the built-in ones. A rule is a YAML file
that describes GDScript code to find, the message to show, and optionally a
fix. You don't compile anything: add a file to `.gdguard/rules/`, and
`gdguard lint` picks it up.

Rules use a subset of [ast-grep](https://ast-grep.github.io/)'s rule format,
so ast-grep knowledge carries over.

Every example on this page was run with gdguard. Outputs are copied from real
runs; absolute paths are shortened to `<project>` and written with `/`.

- [Quick start](#quick-start)
- [The rule file](#the-rule-file)
- [Patterns and metavariables](#patterns-and-metavariables)
- [Node paths and the `$` escape](#node-paths-and-the--escape)
- [Rule operators](#rule-operators)
- [Constraints](#constraints)
- [Fixes and safety](#fixes-and-safety)
- [Testing rules with `test-rules`](#testing-rules-with-test-rules)
- [Debugging a rule](#debugging-a-rule)
- [The starter rule pack](#the-starter-rule-pack)

## Quick start

`gdguard init` writes a `gdguard.toml` and one example rule:

```console
$ gdguard init
Created <project>/gdguard.toml
Created <project>/.gdguard/rules/prefer-is-empty.yml
Run `gdguard lint` to check the project, and `gdguard test-rules` to test the rules in .gdguard/rules.
```

`gdguard.toml` tells gdguard where the rules are:

```toml
[custom_rules]
paths = [".gdguard/rules"]
```

Given this script:

```gdscript
extends Node

var items = []


func _process(delta):
	if len(items) == 0:
		return
	get_node("Player").hide()
```

the custom rule reports alongside the built-in ones:

```console
$ gdguard lint .
scripts/player.gd:6:15: warning [unused-argument] Function argument 'delta' is unused. Consider removing it or prefixing with '_'
scripts/player.gd:7:5: warning [prefer-is-empty] Use `items.is_empty()` instead of comparing `len(items)` to zero
```

The rule has a safe fix, so `--fix` applies it:

```console
$ gdguard lint --diff .
--- a/scripts/player.gd
+++ b/scripts/player.gd
@@ -4,6 +4,6 @@
 
 
 func _process(delta):
-	if len(items) == 0:
+	if items.is_empty():
 		return
 	get_node("Player").hide()
$ gdguard lint --fix .
scripts/player.gd:6:15: warning [unused-argument] Function argument 'delta' is unused. Consider removing it or prefixing with '_'
Fixed 1 problem in 1 file
```

The remaining issues go to stdout and `Fixed ...` goes to stderr, so
`--fix --format json` stays valid JSON.

## The rule file

One rule per file, in a folder listed in `[custom_rules].paths`. gdguard
loads every `*.yml` and `*.yaml` file in the folder itself (not in
sub-folders), in path order.

```yaml
id: no-player-hide                  # required, kebab-case, unique
severity: warning                   # error | warning | info (default: warning)
message: "Use `$Player.set_visible(false)` so the change is easy to search for"  # required
help: "Replace `$Player.hide()` with `$Player.set_visible(false)`."              # optional
rationale: |                        # optional, shown by `gdguard explain`
  Searching for set_visible finds every place that changes visibility.
rule:                               # required: what to find
  pattern: $Player.hide()
fix: $Player.set_visible(false)     # optional rewrite
fixSafety: safe                     # safe | unsafe | suggestion (default: suggestion)
tests:                              # run by `gdguard test-rules`
  invalid:
    - code: "func f():\n\t$Player.hide()\n"
      output: "func f():\n\t$Player.set_visible(false)\n"
  valid:
    - "func f():\n\t$Enemy.hide()\n"
    - "func f():\n\t$Player.show()\n"
```

| Key | Required | Meaning |
|-----|----------|---------|
| `id` | yes | Kebab-case, unique across all rule files, and not a built-in rule id. Used in output, `gdguard.toml` and ignore comments. |
| `severity` | no | Default level. `[rules]` in `gdguard.toml` overrides it, like for built-in rules. |
| `message` | yes | What is wrong. May use metavariables. |
| `help` | no | How to fix it. May use metavariables. |
| `rationale` | no | Why the rule exists, shown by `gdguard explain`. |
| `rule` | yes | The [rule object](#rule-operators). |
| `constraints` | no | Extra conditions on metavariables ([Constraints](#constraints)). |
| `fix` | no | Replacement for the matched code ([Fixes](#fixes-and-safety)). |
| `fixSafety` | no | Needs `fix`. Which `--fix` mode may apply it. |
| `tests` | no | `valid` and `invalid` examples ([Testing](#testing-rules-with-test-rules)). |

Unknown keys are errors, so a typo like `mesage:` never fails silently.

The rule's level is set like any built-in rule:

```toml
[rules]
no-player-hide = "error"
prefer-is-empty = "off"
```

`gdguard explain <id>` and `gdguard lint --list-rules` show custom rules too.
`--list-rules` shows each rule's default severity, not the level set in
`gdguard.toml`.

## Patterns and metavariables

A `pattern` is GDScript code. It matches code with the same syntax tree,
ignoring whitespace and comments. Metavariables are holes in the pattern:

| Syntax | Matches |
|--------|---------|
| `$X` | exactly one node, captured as `X` |
| `$$$ARGS` | zero or more nodes (for example arguments), captured as `ARGS` |
| `$_` | one node, not captured |
| `$$$` | zero or more nodes, not captured |

A metavariable name is uppercase letters, digits and `_`, and must not be
followed by another letter: `$X`, `$ARGS`, `$NODE_1`.

The same name used twice must match the same code: `$A == $A` matches
`x == x` but not `x == y`.

Captures can be used in `message`, `help` and `fix`. `$$$ARGS` renders as the
captured code including the separators between the nodes:

```yaml
rule:
  pattern: emit_signal($NAME, $$$ARGS)
fix: Signal(self, $NAME).emit($$$ARGS)
```

turns `emit_signal("died", 1)` into `Signal(self, "died").emit(1)`.

A pattern must be valid GDScript on its own (`if :` or `func (` are rejected
when the rule loads). Strings, node paths, `&"string names"` and `^"node
paths"` in a pattern match by their exact text: `get_node("Player")` doesn't
match `get_node("Enemy")`.

## Node paths and the `$` escape

GDScript uses `$` for node paths too. Every `$` that doesn't start a
metavariable is literal:

| In a rule | Means |
|-----------|-------|
| `$Player`, `$player`, `$"UI/Label"`, `%Health` | node paths (literal GDScript) |
| `$PLAYER`, `$HUD` | metavariables (all caps) |
| `\$HUD` | the node path `$HUD` |

These rules apply to `pattern`, `fix`, `message` and `help` alike. So
`pattern: $Player.hide()` matches `$Player.hide()` and not `$Enemy.hide()`,
and `fix: $Player.set_visible(false)` writes the node path back.

To match several node paths, capture them and filter with a constraint:

```yaml
rule:
  pattern: $P.hide()
constraints:
  P:
    regex: ^\$(Player|Enemy)$
```

## Rule operators

A rule object combines these keys. All keys in one object must match.

| Key | Kind | Matches a node that... |
|-----|------|------------------------|
| `pattern` | atomic | has the shape of the GDScript code |
| `kind` | atomic | has this tree-sitter node kind (`call`, `function_definition`, ...) |
| `regex` | atomic | has source text matching the regular expression |
| `inside` | relational | is inside a node matching the sub-rule |
| `has` | relational | contains a node matching the sub-rule |
| `follows` | relational | comes after a sibling matching the sub-rule |
| `precedes` | relational | comes before a sibling matching the sub-rule |
| `all` | composite | matches every rule in the list |
| `any` | composite | matches at least one rule in the list |
| `not` | composite | doesn't match the sub-rule |

The top-level `rule` needs at least one of `pattern`, `kind`, `regex`, `all`
or `any`; a rule made only of `not` or relational keys would match almost
every node.

Relational rules also take:

- `stopBy`: how far to look. `neighbor` (default) only looks at the direct
  parent (`inside`), the direct children (`has`) or the next sibling
  (`follows`/`precedes`); `end` looks all the way; a rule object stops at the
  first node matching it.
- `field`: only look at the child in this grammar field, for example the
  `name` of a `function_definition`.

`pattern` also has an object form, `{ context, selector }`: parse `context`
as code and use the node of kind `selector` inside it as the pattern.

Other ast-grep keys (`nthChild`, `range`, `matches`, `utils`, `transform`,
`rewriters`) are not supported.

### `kind` and `regex`

```yaml
id: op-kind-regex
message: "Signal names should be in the past tense"
rule:
  kind: signal_statement
  regex: ^signal (on_|do_)
tests:
  invalid:
    - "signal on_hit\n"
  valid:
    - "signal hit\n"
```

### `inside`

```yaml
id: op-inside
message: "Don't create timers every physics frame"
rule:
  pattern: get_tree().create_timer($$$ARGS)
  inside:
    kind: function_definition
    has:
      field: name
      regex: ^_physics_process$
    stopBy: end
tests:
  invalid:
    - "func _physics_process(delta):\n\tif delta > 0:\n\t\tget_tree().create_timer(1.0)\n"
  valid:
    - "func _ready():\n\tget_tree().create_timer(1.0)\n"
```

### `has` and `not`

```yaml
id: op-has-not
message: "This script connects signals but never disconnects them"
rule:
  kind: source
  has:
    pattern:
      context: $S.connect($$$A)
      selector: attribute_call
    stopBy: end
  not:
    has:
      pattern:
        context: $S.disconnect($$$A)
        selector: attribute_call
      stopBy: end
tests:
  invalid:
    - "func _ready():\n\tbutton.pressed.connect(_on_pressed)\n"
  valid:
    - "func _ready():\n\tbutton.pressed.connect(_on_pressed)\n\nfunc _exit_tree():\n\tbutton.pressed.disconnect(_on_pressed)\n"
```

`kind: source` is the whole file. The `selector: attribute_call` is explained
in [Debugging](#attribute-chains).

### `follows` and `precedes`

They compare siblings, so for statements select the statement:

```yaml
id: op-follows
message: "Code after `queue_free()` still runs this frame"
rule:
  pattern:
    context: queue_free()
    selector: expression_statement
  precedes:
    kind: expression_statement
tests:
  invalid:
    - "func die():\n\tqueue_free()\n\temit_signal(\"died\")\n"
  valid:
    - "func die():\n\temit_signal(\"died\")\n\tqueue_free()\n"
```

`follows` is the mirror: the matched statement comes after one matching the
sub-rule.

### `all` and `any`

```yaml
id: op-any-all
message: "Use `is_zero_approx()` to compare floats with zero"
rule:
  all:
    - kind: binary_operator
    - any:
        - pattern: $X == 0.0
        - pattern: 0.0 == $X
fix: is_zero_approx($X)
fixSafety: unsafe
tests:
  invalid:
    - code: "func f(speed):\n\tif speed == 0.0:\n\t\tpass\n"
      output: "func f(speed):\n\tif is_zero_approx(speed):\n\t\tpass\n"
  valid:
    - "func f(count):\n\tif count == 0:\n\t\tpass\n"
```

## Constraints

`constraints` adds a rule per metavariable. The match only counts when each
captured node also matches its constraint:

```yaml
rule:
  pattern: load($PATH)
constraints:
  PATH:
    kind: string
    regex: ^"res://
```

matches `load("res://enemy.tscn")` but not `load(path)` or
`load("user://save.tres")`.

When a metavariable is captured in more than one place (for example in the
pattern and again inside a `has`), the constraint checks the **last**
capture. In `examples/rules/prefer-is-instance-valid.yml`, `$X` is captured by
`$X != null` (an `identifier`) and then by the declaration's name (a `name`
node), so a `kind: identifier` constraint on `X` would never match.

## Fixes and safety

`fix` replaces the matched code. Captures are filled in; everything else is
literal GDScript.

| `fixSafety` | Applied by | Use when |
|-------------|------------|----------|
| `safe` | `--fix` | The fix never changes behavior. |
| `unsafe` | `--fix-unsafe` | The fix is almost always right, but there are edge cases. |
| `suggestion` (default) | never automatically | A person or agent should decide. Shown in JSON output. |

Things to know:

- **One template per rule.** When two shapes need different fixes (`len(x)
  == 0` vs `len(x) > 0`), write two rules.
- **Templates can't change captured text.** A captured string keeps its
  quotes: `$NAME.emit()` with `NAME = "died"` gives `"died".emit()`, which is
  wrong. Use a form that takes the string, like `Signal(self, $NAME)`.
- **Precedence.** `fix: $X.is_empty()` with `$X` = `a + b` would give
  `a + b.is_empty()`. Limit `$X` with a constraint (see
  `examples/rules/prefer-is-empty.yml`) or put parentheses in the template.
- **Multi-line fixes.** Write the template without the surrounding
  indentation; every line after the first gets the indentation of the line
  the match starts on, with tabs or spaces:

  ```yaml
  rule:
    pattern:
      context: print($A)
      selector: expression_statement
  fix: |-
    if OS.is_debug_build():
    	print($A)
  ```

- **A fix edits only the matched code.** It can't delete the whole line or
  add code elsewhere (like an `@onready` variable at the top of the script).
  Replace a statement with `pass` to remove it safely, or give no fix and
  explain the change in `help`.
- **Fixes never break code.** If a fix makes the file unparsable, `--fix`
  rolls it back and reports `fix-failed` instead.

## Testing rules with `test-rules`

`tests` in a rule file are examples that must behave as stated:

- each `valid` entry must produce no diagnostic for this rule;
- each `invalid` entry must produce at least one, or exactly `errors: N`;
- when an `invalid` entry has `output`, the rule's fix is applied (whatever
  its safety) and the result must equal `output`, must parse, and must not be
  reported again by the rule.

`invalid` entries can be a plain string when there is no output to check.

```console
$ gdguard test-rules
✓ no-player-hide  3 passed
✓ prefer-is-empty  7 passed
✓ prefer-signal-emit  2 passed

3 rules, 3 passed, 0 failed
```

`gdguard test-rules [FOLDER]` tests the rules in `FOLDER`, else the
`[custom_rules].paths` of `gdguard.toml`, else `.gdguard/rules`. Exit code 0
means everything passed, 1 that a test failed, 2 that a rule file is
invalid. Run it in CI next to `gdguard lint`.

## Debugging a rule

### The rule doesn't load

Every load error names the file and the key. For example, a message that
uses a metavariable the pattern doesn't capture:

```console
$ gdguard test-rules
Error: <project>/.gdguard/rules/prefer-signal-emit.yml: `message`: uses the metavariable `$SIGNAL`, which the rule's patterns don't define
```

Other errors name keys like `rule.any[1].kinds` (an unknown key in the second
`any` item) or `rule.pattern` (`pattern `func (` doesn't parse as GDScript`).

### A test fails

`test-rules` shows a diff for wrong fix output (`-` expected, `+` actual):

```console
$ gdguard test-rules
✓ no-player-hide  3 passed
✓ prefer-is-empty  7 passed
✗ prefer-signal-emit  1 failed, 1 passed
  invalid[0]: the fixed code differs from `output` (- expected, + actual):
    | func f():
    - 	died.emit(1)
    + 	"died".emit(1)

3 rules, 2 passed, 1 failed
```

Here the template was `$NAME.emit($$$ARGS)`, and `NAME` captured the string
with its quotes (see [Fixes](#fixes-and-safety)).

For an invalid case that isn't reported, `test-rules` prints the code; for a
valid case that is reported, it also prints each diagnostic's line, column
and message.

### Common reasons a rule doesn't match

- **The pattern selects a different node than you think.** A pattern like
  `queue_free()` selects the `call`, not the statement around it. That
  matters for `follows`/`precedes` (siblings are statements) and for fixes.
  Use `pattern: { context: ..., selector: expression_statement }`.
- <a id="attribute-chains"></a>**Attribute chains are flat.** In
  tree-sitter-gdscript, `button.pressed.connect(f)` is one `attribute` node
  with three parts, so the pattern `$S.connect($$$A)` (two parts) doesn't
  match it. Select the call part instead:
  `pattern: { context: $S.connect($$$A), selector: attribute_call }`.
- **`$HUD` is a metavariable.** Use `\$HUD` for an all-caps node path.
- **`stopBy` defaults to `neighbor`.** `inside` without `stopBy: end` only
  looks at the direct parent.
- **Constraints see the last capture** of a metavariable ([Constraints](#constraints)).
- **Node kinds.** `kind` uses tree-sitter-gdscript names: `call`,
  `attribute`, `attribute_call`, `function_definition`, `variable_statement`,
  `if_statement`, `for_statement`, `get_node`, `string`, `identifier`,
  `name` (a declared name), `source` (the file), and so on. The grammar's
  `src/node-types.json` lists them all.

## The starter rule pack

`examples/rules/` has tested rules to copy into `.gdguard/rules/`:

| Rule | Finds | Fix |
|------|-------|-----|
| `prefer-is-empty` | `len(x) == 0`, `len(x) < 1` | `x.is_empty()` (safe) |
| `prefer-not-is-empty` | `len(x) > 0`, `!= 0`, `>= 1` | `not x.is_empty()` (safe) |
| `no-get-node-in-process` | `get_node()`/`$Path` in `_process`/`_physics_process` | none |
| `prefer-preload-const` | `load("res://...")` in loops or `_process` | `preload(...)` (suggestion) |
| `no-godot3-yield` | `yield(obj, "signal")` | `await Signal(obj, "signal")` (unsafe) |
| `no-string-connect` | Godot 3 `connect("sig", target, "method")` | `connect("sig", Callable(target, "method"))` (unsafe) |
| `no-print-left` | `print()`, `prints()`, `print_debug()` statements | `pass` (suggestion) |
| `prefer-is-instance-valid` | `x != null` on a variable declared with a node type | `is_instance_valid(x)` (suggestion) |

```console
$ gdguard test-rules examples/rules
✓ no-get-node-in-process  7 passed
✓ no-godot3-yield  6 passed
✓ no-print-left  6 passed
✓ no-string-connect  6 passed
✓ prefer-is-empty  7 passed
✓ prefer-is-instance-valid  7 passed
✓ prefer-not-is-empty  7 passed
✓ prefer-preload-const  7 passed

8 rules, 8 passed, 0 failed
```
