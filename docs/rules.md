# Built-in rules

gdguard has 29 built-in rules. This page lists them all, grouped by what they catch. The same text is available offline:

```sh
gdguard lint --list-rules     # one line per rule
gdguard explain <rule>        # why, and bad/good examples
```

Change a rule's level, or turn it off, in `gdguard.toml` ([configuration](configuration.md)):

```toml
[rules]
missing-class-name = "off"
unused-argument = "info"
max-line-length = { level = "warning", max = 120 }
```

Levels: **error** and **warning** fail the run (warnings up to `--max-warnings N` are allowed); **info** never does. `lint --fix` applies **safe** fixes, `--fix-unsafe` also applies **unsafe** ones, and **suggestions** are never applied automatically. To have `lint --fix` (and the agent hook) apply some unsafe fixes too, list their rules: `[fix] unsafe = ["prefer-squared-distance"]`, or `unsafe = true` for all. Silence one line with `# gdguard-ignore-next-line <rule> -- reason`.

## Consolidated rules and compatibility

Related checks share one rule name in reports and the catalog. `naming-convention` combines ten naming checks, `unused-code` combines seven unused-code checks, and `redundant-expression` combines three expression checks. `type-annotations` covers the typing policy in one rule. Other checks remain separate where their purpose or severity differs.

Old check IDs still work in config, overrides, `--disable`, ignore comments, `explain`, unsafe-fix allowlists and existing baselines. JSON diagnostics for consolidated rules include an optional `check` field with the granular ID. Within one config table, a granular setting wins over its group; a later folder override wins over earlier settings. A group ignore suppresses the whole group on that line, while an old check ignore remains specific.

## All rules

| Rule | Level | Fix | Group |
| --- | --- | --- | --- |
| [`type-annotations`](#type-annotations) | warning |  | Typing |
| [`redundant-expression`](#redundant-expression) | warning |  | Likely bugs |
| [`unused-code`](#unused-code) | warning | safe/unsafe/suggestion | Unused code |
| [`naming-convention`](#naming-convention) | error |  | Naming and style |
| [`parse-error`](#parse-error) | error |  | Syntax and Godot 4 compatibility |
| [`godot3-syntax`](#godot3-syntax) | error | safe/unsafe | Syntax and Godot 4 compatibility |
| [`single-line-container-type`](#single-line-container-type) | error | safe | Syntax and Godot 4 compatibility |
| [`unreachable-code`](#unreachable-code) | warning |  | Likely bugs |
| [`duplicate-dictionary-key`](#duplicate-dictionary-key) | error |  | Likely bugs |
| [`shadowed-variable`](#shadowed-variable) | warning |  | Likely bugs |
| [`private-access`](#private-access) | error |  | Likely bugs |
| [`unused-ignore`](#unused-ignore) | warning | safe | Unused code |
| [`duplicate-class-name`](#duplicate-class-name) | error |  | Project rules |
| [`missing-class-name`](#missing-class-name) | warning | unsafe | Project rules |
| [`scene-connection-missing-method`](#scene-connection-missing-method) | error |  | Project rules |
| [`missing-resource-path`](#missing-resource-path) | error |  | Project rules |
| [`prefer-uid-path`](#prefer-uid-path) | warning | safe | Project rules |
| [`unknown-member-on-project-class`](#unknown-member-on-project-class) | warning |  | Project rules |
| [`prefer-unique-node-name`](#prefer-unique-node-name) | info | unsafe | Project rules |
| [`declaration-order`](#declaration-order) | warning | safe/unsafe | Naming and style |
| [`max-line-length`](#max-line-length) | warning |  | Naming and style |
| [`no-else-return`](#no-else-return) | warning | safe/unsafe | Naming and style |
| [`duplicated-load`](#duplicated-load) | warning |  | Naming and style |
| [`prefer-is-empty-call`](#prefer-is-empty-call) | info | safe | Naming and style |
| [`prefer-squared-distance`](#prefer-squared-distance) | info | unsafe | Naming and style |
| [`prefer-array-method`](#prefer-array-method) | info | unsafe | Naming and style |
| [`fix-failed`](#fix-failed) | warning |  | Tool messages |
| [`script-error`](#script-error) | error |  | Tool messages |
| [`godot-error`](#godot-error) | error |  | Tool messages |

## Typing

### `type-annotations`

**warning**

Functions, parameters, variables, constants, signals and containers declare types.

Explicit types document contracts and let Godot check them. Includes function and lambda return types (including void), parameters, variables, constants, for-loop variables, signal parameters and Array/Dictionary arguments. Property accessors inherit their property's type. Explicit Variant is allowed for intentional dynamic values. With allow_inferred = true, := declarations and parameters are accepted without proving their inferred type. Types are never guessed or inserted automatically.

Bad:

```gdscript
func f():
	pass
```

Good:

```gdscript
func f() -> void:
	pass
```

The default requires explicit annotations. To allow `:=` for declarations and default parameters:

```toml
[rules]
type-annotations = { level = "warning", allow_inferred = true }
```

This option permits inference syntax; it does not verify that Godot can infer a useful type. Untyped `var x = 1` still reports. Functions and lambdas still need return types and loops need element annotations. Empty signals need no parameter annotations. Getter/setter types come from their property. `Variant` is an intentional escape hatch, and packed arrays already specify their element type. No fixes are offered.

## Syntax and Godot 4 compatibility

Code that Godot 4 can't load.

### `parse-error`

**error**

The source parses as GDScript.

A syntax error prevents reliable analysis. Repair the reported range and lint again; use Godot for additional parser and type checks.

Bad:

```gdscript
func f(:
```

Good:

```gdscript
func f():
	pass
```

### `godot3-syntax`

**error** · fix: safe for `tool` and `onready`; unsafe for `export var`, `yield`, `instance()` and `connect`

No Godot 3 syntax that Godot 4 rejects.

Code copied from Godot 3 examples uses `tool`, `onready`, `export`, `setget`, `yield`, `instance()` and string-method `connect`. Godot 4 rejects or renamed them. Mechanical replacements are offered as fixes (`tool` and `onready` safe, the others unsafe).

Bad:

```gdscript
func _ready():
	yield(get_tree(), "idle_frame")
```

Good:

```gdscript
func _ready():
	await get_tree().process_frame
```

### `single-line-container-type`

**error** · fix: safe (joins the type on one line)

Array[...] and Dictionary[...] types stay on one line.

Godot 4.7 rejects a line break inside the brackets of a variable, constant, return or loop-variable type ("Expected type for collection after ..."), but the bundled parser accepts it, so `parse-error` doesn't catch it. Parameter and lambda types, and `\` continuations, are accepted by Godot and not reported. Types with a comment inside get no fix.

Bad:

```gdscript
func living() -> Array[
	Node
]:
	return []
```

Good:

```gdscript
func living() -> Array[Node]:
	return []
```

## Likely bugs

Code that runs but probably doesn't do what was meant.

### `redundant-expression`

**warning**

Expressions have an effect and don't compare or assign a value to itself.

Combines comparison-with-itself, self-assignment and standalone-expression. These commonly indicate a typo, but the intended correction cannot be inferred. Member self-assignment is exempt because it may invoke a setter. Old check names remain accepted in config and ignores.

Bad:

```gdscript
func add_point(score: int) -> int:
	score + 1
	return score
```

Good:

```gdscript
func add_point(score: int) -> int:
	score += 1
	return score
```

The following compatibility checks describe the individual cases.

#### `comparison-with-itself`

**warning**

Comparing an expression with itself is always true or always false.

`x == x` or `x != x` has a fixed result, so it is almost always a typo for a comparison with another value.

Bad:

```gdscript
func is_target(target, other):
	return target == target
```

Good:

```gdscript
func is_target(target, other):
	return target == other
```

#### `self-assignment`

**warning**

Local variables and parameters aren't assigned to themselves.

`x = x` on a local has no effect and is usually a typo for another value. Members are skipped: `property = property` is a Godot idiom to run a setter again.

Bad:

```gdscript
func set_speed(speed):
	speed = speed
```

Good:

```gdscript
var speed = 0
func set_speed(value):
	speed = value
```

#### `standalone-expression`

**warning**

Expressions on their own line have an effect.

A line like `score + 1` computes a value and throws it away. It is usually a mistake for `score += 1` or a leftover.

Bad:

```gdscript
func add_point(score):
	score + 1
	return score
```

Good:

```gdscript
func add_point(score):
	score += 1
	return score
```

### `unreachable-code`

**warning**

No statements after `return`, `break` or `continue` in the same block.

Code after these statements never runs. Godot only warns about it in the editor, so headless checks and agents miss it.

Bad:

```gdscript
func f():
	return 1
	print("done")
```

Good:

```gdscript
func f():
	print("done")
	return 1
```

### `duplicate-dictionary-key`

**error**

Dictionary literals don't repeat a key.

Only the last value of a repeated key is kept, so the earlier entry is dead and usually a typo. Godot refuses to load scripts with duplicate literal keys.

Bad:

```gdscript
var stats = {"hp": 10, "mp": 5, "hp": 20}
```

Good:

```gdscript
var stats = {"hp": 10, "mp": 5, "sp": 20}
```

### `shadowed-variable`

**warning**

Local names do not shadow outer declarations.

Shadowing makes it easy to read or assign a different variable than intended.

Bad:

```gdscript
var value = 1
func f():
	var value = 2
	print(value)
```

Good:

```gdscript
var value = 1
func f():
	var local_value = 2
	print(local_value)
```

### `private-access`

**error**

Members starting with `_` are not used from outside their class.

A leading underscore marks a member as private. Using it from another object couples the two classes to internals that may change.

Bad:

```gdscript
func damage(enemy):
	enemy._health -= 1
```

Good:

```gdscript
func damage(enemy):
	enemy.take_damage(1)
```

## Unused code

Dead code and leftovers.

### `unused-code`

**warning** · fix: safe/unsafe/suggestion (depends on the check)

Variables, arguments, private functions and signals are used; redundant pass is removed.

Combines unused variables, arguments, assigned-but-never-read values, private functions, local/project signals and unnecessary-pass. Project signal checks include script and scene connections, with a local fallback outside a project. Fix safety is per diagnostic: redundant pass removal is safe, variable removal and argument renaming unsafe, private-function removal suggestion-only; other checks have no fix. Old check names remain accepted in config, ignores, unsafe-fix allowlists and baselines.

Bad:

```gdscript
func f() -> void:
	var value: int = 1
```

Good:

```gdscript
func f() -> void:
	var value: int = 1
	print(value)
```

The following compatibility checks retain their individual fix safety.

#### `unused-variable`

**warning** · fix: unsafe (removes the declaration)

Local variables are used or start with an underscore.

Unused locals can hide mistakes or unnecessary work. Removing a declaration can affect behavior, so the offered fix is unsafe.

Bad:

```gdscript
func f():
	var value = 1
```

Good:

```gdscript
func f():
	var value = 1
	print(value)
```

#### `unused-argument`

**warning** · fix: unsafe (prefixes the name with `_`)

Function arguments are used, or start with `_`.

An unused argument is often a bug or a leftover. When the signature must stay (callbacks, overrides), prefix the name with `_` to say so.

Bad:

```gdscript
func _process(delta):
	print("tick")
```

Good:

```gdscript
func _process(_delta):
	print("tick")
```

#### `assigned-but-never-read`

**warning**

Assignments to local variables are read.

A value repeatedly assigned and never read often means a missing use or assignment to the wrong variable.

Bad:

```gdscript
func f():
	var value = 0
	value = 1
```

Good:

```gdscript
func f():
	var value = 0
	value = 1
	print(value)
```

#### `unused-private-function`

**warning** · fix: suggestion only (never applied automatically)

Private functions are referenced or are Godot virtual callbacks.

A private helper with no local references may be dead code. Check external callbacks before removing it.

Bad:

```gdscript
func _helper():
	pass
```

Good:

```gdscript
func _helper():
	pass
func f():
	_helper()
```

#### `unused-local-signal`

**warning**

Signals are emitted or connected in their declaring file.

Unused signals can be leftovers; external or scene-only usage requires a documented exception.

Bad:

```gdscript
signal changed
```

Good:

```gdscript
signal changed
func f():
	changed.emit()
```

#### `unnecessary-pass`

**warning** · fix: safe

No `pass` in a block that has other statements.

`pass` only exists to fill an empty block. Next to other statements it does nothing.

Bad:

```gdscript
func f():
	print(1)
	pass
```

Good:

```gdscript
func f():
	print(1)
```

### `unused-ignore`

**warning** · fix: safe

Ignore comments suppress an existing issue.

An unused ignore can hide future issues. Remove the comment or the rule ids that no longer apply. Disabled rules are exempt, since they cannot produce an issue.

Bad:

```gdscript
# gdguard-ignore-next-line unused-argument -- old callback
func f():
	pass
```

Good:

```gdscript
func f():
	pass
```

## Project rules

These need `project.godot`: they read other scripts, scenes, `.uid` and `.import` files. Outside a Godot project they don't run.

### `duplicate-class-name`

**error**

Each class_name is declared by one script in the project.

Godot registers one global class per name. The other script fails to load with `Class "X" hides a global script class`, and the editor doesn't always say which file wins.

Bad:

```gdscript
class_name ProjectData
extends Node
```

Good:

```gdscript
class_name ExampleData
extends Node
```

### `missing-class-name`

**warning** · fix: unsafe (adds `class_name` from the file name)

Every project script declares a class_name, except autoloads.

A class_name lets other scripts use the script as a type, which enables typed code and the cross-file checks. Autoload scripts are exempt: they are reached through their singleton name. The unsafe fix adds a PascalCase name from the file name when it is free.

Bad:

```gdscript
extends Node
```

Good:

```gdscript
class_name Example
extends Node
```

### `scene-connection-missing-method`

**error**

Scene connections target methods the script defines.

A `[connection]` in a `.tscn` file names its method as a string. Godot only reports a missing method when the signal is emitted. Methods inherited from engine classes can't be checked, except that engine methods never start with `_` unless they are virtual callbacks.

Bad:

```gdscript
extends Control
```

Good:

```gdscript
extends Control

func _on_button_pressed():
	pass
```

### `missing-resource-path`

**error**

Paths given to load() and preload() exist in the project.

A wrong `load()` path only fails at runtime, when the line runs. `res://` and relative paths must name an existing file, and `uid://` paths a UID known from `.uid` files, `.import` files or scene headers.

Bad:

```gdscript
var icon = load("res://missing.png")
```

Good:

```gdscript
var data = load("res://data.gd")
```

### `prefer-uid-path`

**warning** · fix: safe (replaces the path with the UID)

load() and preload() use uid:// paths when the resource has a UID.

UID paths keep working when files are moved or renamed. Only paths with a known UID (from `.uid` files, `.import` files or scene headers) are reported, and the safe fix replaces the path with the UID.

Bad:

```gdscript
const Data = preload("res://data.gd")
```

Good:

```gdscript
const Data = preload("uid://cexampledata")
```

#### `unused-signal`

**warning**

Project signals are emitted or connected somewhere in the project.

Unused signals can be removed after reviewing external and dynamic use. Script calls and scene connections count as use.

Bad:

```gdscript
signal unused
```

Good:

```gdscript
signal used
func f():
	used.emit()
```

### `unknown-member-on-project-class`

**warning**

Declared project types refer to existing project members.

A missing member on a known project type is often a typo. Engine, dynamic and inferred types remain unknown.

Bad:

```gdscript
func f(data: ProjectData):
	return data.missing
```

Good:

```gdscript
func f(data: ProjectData):
	return data.value
```

### `prefer-unique-node-name`

**info** · fix: unsafe (replaces the path with `%Name`)

Scripts reach their scene's unique nodes with %Name instead of a $path.

`%Button` keeps working when the scene tree is rearranged; `$Panel/Button` breaks. Reported only when the script is the root script of its scenes (and of no other node), and every one of those scenes marks the node `unique_name_in_owner`. Inherited scenes and nodes inside instanced scenes are skipped. The fix is unsafe: the script may also be used where the scan can't see it.

Bad:

```gdscript
func _ready() -> void:
	$Panel/Button.text = "Go"
```

Good:

```gdscript
func _ready() -> void:
	%Button.text = "Go"
```

## Naming and style

The [GDScript style guide](https://docs.godotengine.org/en/stable/tutorials/scripting/gdscript/gdscript_styleguide.html) and readability.

### `declaration-order`

**warning** · fix: safe (unsafe when a member-variable dependency changes order)

Top-level declarations follow the formatter's style-guide order, within each
`#region` group. Inner classes are not checked. One diagnostic names the first
declaration that should move earlier, distinguishing regular, exported,
static and onready variables.

The fix keeps attached comments and annotations together and splits inline
`class_name X extends Y`. For formatted files it matches
`gdguard --reorder-code`; for unformatted files it moves original source
slices, preserving declaration bodies, file prefix and suffix, and trailing
comments. Moved items get one blank line between them, two before and after
functions and inner classes. Files with `# fmt: off` regions or top-level
`@warning_ignore_start`/`@warning_ignore_restore` annotations are reported
without a fix; files with parse errors are skipped.

The fix requires `--fix-unsafe` when an initializer references a member variable
that was declared before it but would be declared after it. Bare identifiers
(`var b = a`) and `self.a` count, and all variable kinds are checked even if a
variable keeps the same numeric position. Disable with
`[rules] declaration-order = "off"`.

Bad:

```gdscript
var health = 100
signal died
```

Good:

```gdscript
signal died
var health = 100
```

### `naming-convention`

**error**

Declarations follow GDScript naming conventions.

Combines class, signal, enum, enum-member, constant, variable, loaded-resource variable, function, argument and loop-variable naming. Classes and enums use PascalCase, constants and enum members CONSTANT_CASE, and other names snake_case with private prefixes where appropriate. Loaded resources also allow PascalCase. Renames require checking references, so no automatic fix is offered. Old check names remain accepted in config and ignores.

Bad:

```gdscript
var targetName: String = ""
```

Good:

```gdscript
var target_name: String = ""
```

The following compatibility checks describe each naming convention.

#### `class-name`

**error**

Class names use PascalCase.

The GDScript style guide names classes in PascalCase, like Godot's own classes. A consistent case tells types apart from variables and functions at a glance.

Bad:

```gdscript
class_name player_controller
extends Node
```

Good:

```gdscript
class_name PlayerController
extends Node
```

#### `signal-name`

**error**

Signals use snake_case.

The GDScript style guide names signals in snake_case, usually in the past tense, like Godot's built-in signals.

Bad:

```gdscript
signal OnDefeated
```

Good:

```gdscript
signal defeated
```

#### `enum-name`

**error**

Enum names use PascalCase.

Enums are types, so the GDScript style guide names them in PascalCase like classes.

Bad:

```gdscript
enum state { IDLE, RUN }
```

Good:

```gdscript
enum State { IDLE, RUN }
```

#### `enum-member-name`

**error**

Enum members use CONSTANT_CASE.

Enum members are constants, so the GDScript style guide names them in CONSTANT_CASE.

Bad:

```gdscript
enum State { idle, chase_player }
```

Good:

```gdscript
enum State { IDLE, CHASE_PLAYER }
```

#### `constant-name`

**error**

Constants use CONSTANT_CASE (preloaded constants may use PascalCase).

The GDScript style guide names constants in CONSTANT_CASE so they stand out from variables. Constants holding a preloaded resource may also use PascalCase, like the class they stand for.

Bad:

```gdscript
const maxSpeed = 10
```

Good:

```gdscript
const MAX_SPEED = 10
const Bullet = preload("res://bullet.tscn")
```

#### `variable-name`

**error**

Variables use snake_case (or _private_snake_case).

The GDScript style guide names variables in snake_case. A leading underscore marks private variables.

Bad:

```gdscript
var targetName = ""
```

Good:

```gdscript
var target_name = ""
```

#### `load-variable-name`

**error**

Variables holding a loaded resource use PascalCase, snake_case or _private_snake_case.

A variable that holds a loaded scene or script stands for a type, so PascalCase is allowed in addition to the usual variable cases. This id is reported by the `variable-name` rule and is turned on and off with it.

Bad:

```gdscript
var player_SCENE = load("res://player.tscn")
```

Good:

```gdscript
var PlayerScene = load("res://player.tscn")
```

#### `function-name`

**error**

Functions use snake_case (or _private_snake_case).

The GDScript style guide names functions in snake_case, like Godot's own API. A leading underscore marks private functions and virtual callbacks.

Bad:

```gdscript
func moveTowards(target):
	print(target)
```

Good:

```gdscript
func move_towards(target):
	print(target)
```

#### `function-argument-name`

**error**

Function arguments use snake_case.

The GDScript style guide names arguments in snake_case, like variables, so code reads the same everywhere.

Bad:

```gdscript
func move(TargetPosition):
	print(TargetPosition)
```

Good:

```gdscript
func move(target_position):
	print(target_position)
```

#### `loop-variable-name`

**error**

Loop variables use snake_case.

Loop variables are variables, so the GDScript style guide names them in snake_case.

Bad:

```gdscript
func f(items):
	for Item in items:
		print(Item)
```

Good:

```gdscript
func f(items):
	for item in items:
		print(item)
```

### `max-line-length`

**warning**

Lines are no longer than the configured maximum (100 by default).

Long lines are hard to read side by side and in diffs. The limit comes from `max-line-length.max` in gdguard.toml, then `max_line_length` in .editorconfig, then 100.

Bad:

```gdscript
var message = "This line is far too long because it keeps going and going well past the one hundred character limit"
```

Good:

```gdscript
var message = (
	"This line is split so that every part "
	+ "stays under the limit"
)
```

### `no-else-return`

**warning** · fix: safe (unsafe when the `else` body declares a variable)

No `else`/`elif` after an `if` block that ends with `return`.

When the `if` branch returns, the code after it only runs in the other case anyway. Removing the `else` reduces nesting.

Bad:

```gdscript
func sign_of(x):
	if x < 0:
		return -1
	else:
		return 1
```

Good:

```gdscript
func sign_of(x):
	if x < 0:
		return -1
	return 1
```

### `duplicated-load`

**warning**

The same resource path is loaded more than once in a file.

Loading the same path in several places repeats the string and makes renames error-prone. Load it once into a constant and reuse it.

Bad:

```gdscript
var a = load("res://icon.svg")
var b = load("res://icon.svg")
```

Good:

```gdscript
const ICON = preload("res://icon.svg")
var a = ICON
var b = ICON
```

### `prefer-is-empty-call`

**info** · fix: safe

Emptiness checks on arrays, dictionaries and strings use is_empty().

`items.is_empty()` states the intent better than `items.size() > 0` or `len(items) == 0`. Handles `size()`, `length()` (String) and `len()` compared with `0` or `1`, in either order. Only identifiers whose type is declared, or set from a literal like `[]`, `{}` or `""`, as an Array, Dictionary, packed array or String are reported, so the fix never changes behavior. A project class with its own `size()` stays silent. The built-in `prefer-is-empty` example custom rule from `gdguard init` is separate.

Bad:

```gdscript
func has_items(items: Array) -> bool:
	return items.size() > 0
```

Good:

```gdscript
func has_items(items: Array) -> bool:
	return not items.is_empty()
```

### `prefer-squared-distance`

**info** · fix: unsafe

Distance and length comparisons on vectors use the squared forms.

`distance_squared_to()` and `length_squared()` skip a square root, which matters in per-frame range checks. Receivers must be known float vectors: identifiers declared (or set from a literal) as `Vector2`, `Vector3` or `Vector4`, inherited `position`/`global_position`, `other.global_position`, and sums or differences of those. The threshold must be free of calls so it can be squared without running anything twice; integer literals are squared (`40` → `1600`). Equality checks and real-distance arithmetic are left alone. The fix is unsafe: a negative threshold, or rounding exactly on the boundary, can change the result.

Bad:

```gdscript
func near(a: Vector2, b: Vector2) -> bool:
	return a.distance_to(b) < 40.0
```

Good:

```gdscript
func near(a: Vector2, b: Vector2) -> bool:
	return a.distance_squared_to(b) < 40.0 * 40.0
```

### `prefer-array-method`

**info** · fix: unsafe

Loops that rebuild any(), all(), filter(), map() or a copy use the Array method.

`for e in items: if P: return true` followed by `return false` becomes `return items.any(func(e): return P)` (`all` with `not P` when reversed). A loop that appends to an empty array declared right before it becomes `result.assign(items.filter(...))`, `result.assign(items.map(...))`, or `result.assign(items)` for a plain copy. The source must be an identifier known to be an `Array`; the lambda parameter keeps the loop variable's type, or the array's element type. Loops with comments, `await`, assignments, nested loops or functions, or conditions over several lines aren't reported. The fix is unsafe: the lambda captures local variables by value.

Bad:

```gdscript
func any_ready(items: Array[Node]) -> bool:
	for item in items:
		if item.is_node_ready():
			return true
	return false
```

Good:

```gdscript
func any_ready(items: Array[Node]) -> bool:
	return items.any(func(item: Node): return item.is_node_ready())
```

## Tool messages

Reported by gdguard itself rather than found in the code. You can set their levels like any rule.

### `fix-failed`

**warning**

A fix was not applied because it would break the code.

The fix engine re-parses the code after each pass. When a fix produces code that doesn't parse, it is rolled back and reported under this id instead, so `--fix` never writes broken code. Fix the original issue by hand; if the fix comes from a custom rule, check its `fix` template.

### `script-error`

**error**

A JavaScript rule failed or exceeded its sandbox limits.

A failing custom rule cannot reliably check this file. Fix the rule, invalid report, or resource usage before trusting the lint result.

### `godot-error`

**error**

Godot loads the script without errors (only with `--godot` or `GDGUARD_GODOT`).

gdguard doesn't check types. With a Godot 4 executable configured, `lint` loads the linted scripts in headless Godot once per project and reports its parse, type and warning-as-error messages under this id.

## Custom rules

Add project rules without rebuilding, as [YAML patterns](custom_rules.md) or [sandboxed JavaScript](js_rules.md). Tested examples are in [examples/rules](../examples/rules/).
