@tool
extends EditorPlugin

const SETTINGS := {
	"gdguard/enabled": true,
	"gdguard/lint_on_save": true,
	"gdguard/show_warnings": true,
}

var _active := false
var _panel: VBoxContainer
var _issues: Tree
var _status: Label
var _apply: Button
var _fix_all: Button
var _timer: Timer
var _script_editor: ScriptEditor
var _code_edit: CodeEdit
var _snapshot := ""
var _path := ""


func _enter_tree() -> void:
	# Headless/agent runs use the CLI or native API; do not create editor UI.
	if DisplayServer.get_name() == "headless":
		return
	_active = true
	for setting: String in SETTINGS:
		if not ProjectSettings.has_setting(setting):
			ProjectSettings.set_setting(setting, SETTINGS[setting])
		ProjectSettings.set_initial_value(setting, SETTINGS[setting])
		ProjectSettings.add_property_info({"name": setting, "type": TYPE_BOOL})
	_panel = VBoxContainer.new()
	_panel.custom_minimum_size = Vector2(0, 180)
	var controls := HBoxContainer.new()
	_panel.add_child(controls)
	_status = Label.new()
	_status.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	controls.add_child(_status)
	var refresh := Button.new()
	refresh.text = "Lint current script"
	refresh.pressed.connect(_lint_current)
	controls.add_child(refresh)
	_apply = Button.new()
	_apply.text = "Apply selected fix"
	_apply.disabled = true
	_apply.pressed.connect(_apply_selected)
	controls.add_child(_apply)
	_fix_all = Button.new()
	_fix_all.text = "Fix all safe"
	_fix_all.pressed.connect(_fix_all_safe)
	controls.add_child(_fix_all)
	_issues = Tree.new()
	_issues.columns = 2
	_issues.hide_root = true
	_issues.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_issues.set_column_title(0, "Position / rule")
	_issues.set_column_title(1, "Message")
	_issues.column_titles_visible = true
	_issues.item_selected.connect(_selection_changed)
	_issues.item_activated.connect(_jump_to_issue)
	_panel.add_child(_issues)
	_timer = Timer.new()
	_timer.one_shot = true
	_timer.wait_time = 0.3
	_timer.timeout.connect(_lint_current)
	add_child(_timer)
	add_control_to_bottom_panel(_panel, "gdguard")
	_script_editor = EditorInterface.get_script_editor()
	_script_editor.editor_script_changed.connect(_script_changed)
	resource_saved.connect(_resource_saved)
	_bind_editor.call_deferred()


func _exit_tree() -> void:
	if not _active:
		return
	_active = false
	if is_instance_valid(_code_edit) and _code_edit.text_changed.is_connected(_text_changed):
		_code_edit.text_changed.disconnect(_text_changed)
	if is_instance_valid(_script_editor) and _script_editor.editor_script_changed.is_connected(_script_changed):
		_script_editor.editor_script_changed.disconnect(_script_changed)
	if resource_saved.is_connected(_resource_saved):
		resource_saved.disconnect(_resource_saved)
	remove_control_from_bottom_panel(_panel)
	_panel.queue_free()
	_timer.stop()


func _backend() -> Object:
	if Engine.has_singleton("GDScriptFormatter"):
		return Engine.get_singleton("GDScriptFormatter")
	return null


func _script_changed(_script: Script) -> void:
	_bind_editor.call_deferred()


func _bind_editor() -> void:
	if is_instance_valid(_code_edit) and _code_edit.text_changed.is_connected(_text_changed):
		_code_edit.text_changed.disconnect(_text_changed)
	_code_edit = null
	var editor := EditorInterface.get_script_editor().get_current_editor()
	if is_instance_valid(editor):
		_code_edit = editor.get_base_editor() as CodeEdit
	if is_instance_valid(_code_edit):
		_code_edit.text_changed.connect(_text_changed)
	_lint_current()


func _text_changed() -> void:
	_timer.start()


func _resource_saved(resource: Resource) -> void:
	if resource is GDScript and ProjectSettings.get_setting("gdguard/lint_on_save", true):
		_lint_current()


func _lint_current() -> void:
	_timer.stop()
	_issues.clear()
	_apply.disabled = true
	_fix_all.disabled = true
	if not ProjectSettings.get_setting("gdguard/enabled", true):
		_status.text = "Lint disabled in project settings"
		return
	var backend := _backend()
	if backend == null:
		_status.text = "Install the gdguard native extension to enable linting"
		return
	var script := EditorInterface.get_script_editor().get_current_script()
	if not script is GDScript or not is_instance_valid(_code_edit):
		_status.text = "Open a GDScript to lint"
		return
	_path = script.resource_path if not script.resource_path.is_empty() else "res://.gdguard_unsaved.gd"
	_snapshot = _code_edit.text
	var diagnostics: Array = backend.call("lint", _path, _snapshot)
	var root := _issues.create_item()
	var visible := 0
	for issue: Dictionary in diagnostics:
		if issue.get("severity") == "warning" and not ProjectSettings.get_setting("gdguard/show_warnings", true):
			continue
		var item := _issues.create_item(root)
		var icon_name := "StatusError" if issue.get("severity") == "error" else "StatusWarning"
		if issue.get("severity") == "info":
			icon_name = "StatusInfo"
		item.set_icon(0, EditorInterface.get_base_control().get_theme_icon(icon_name, "EditorIcons"))
		item.set_text(0, "%d:%d [%s]" % [issue.line, issue.column, issue.rule])
		item.set_text(1, issue.message)
		item.set_tooltip_text(1, issue.get("help", ""))
		item.set_metadata(0, issue)
		visible += 1
	_status.text = "%d issues in %s" % [visible, _path.get_file()]
	_fix_all.disabled = false


func _selection_changed() -> void:
	var selected := _issues.get_selected()
	if selected == null:
		return
	var issue: Dictionary = selected.get_metadata(0)
	var fixes: Array = issue.get("fixes", [])
	_apply.disabled = fixes.is_empty()
	if not fixes.is_empty():
		_apply.text = "Apply fix (%s)" % fixes[0].get("safety", "suggestion")
	_jump_to_issue()


func _jump_to_issue() -> void:
	var selected := _issues.get_selected()
	if selected == null or not is_instance_valid(_code_edit):
		return
	var issue: Dictionary = selected.get_metadata(0)
	var line := clampi(int(issue.line) - 1, 0, _code_edit.get_line_count() - 1)
	var bytes := _code_edit.get_line(line).to_utf8_buffer()
	var column := bytes.slice(0, int(issue.column) - 1).get_string_from_utf8().length()
	_code_edit.set_caret_line(line)
	_code_edit.set_caret_column(column)
	_code_edit.grab_focus()


func _apply_selected() -> void:
	var selected := _issues.get_selected()
	if selected == null or not _snapshot_is_current():
		return
	var issue: Dictionary = selected.get_metadata(0)
	var fixes: Array = issue.get("fixes", [])
	if fixes.is_empty():
		return
	var result: Dictionary = _backend().call("apply_edits", _snapshot, fixes[0].edits)
	_accept_result(result)


func _fix_all_safe() -> void:
	if not _snapshot_is_current():
		return
	var result: Dictionary = _backend().call("fix", _path, _snapshot, false)
	_accept_result(result)


func _snapshot_is_current() -> bool:
	if not is_instance_valid(_code_edit) or _code_edit.text != _snapshot:
		_lint_current()
		return false
	return _backend() != null


func _accept_result(result: Dictionary) -> void:
	if result.has("error"):
		_status.text = result.error
		return
	var line := _code_edit.get_caret_line()
	var column := _code_edit.get_caret_column()
	_code_edit.begin_complex_operation()
	_code_edit.select_all()
	_code_edit.insert_text_at_caret(result.output)
	_code_edit.end_complex_operation()
	_code_edit.set_caret_line(mini(line, _code_edit.get_line_count() - 1))
	_code_edit.set_caret_column(column)
	_lint_current()
