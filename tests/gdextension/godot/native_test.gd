extends SceneTree

var failures := 0


func check(condition: bool, message: String) -> void:
	if not condition:
		failures += 1
		push_error(message)


func _initialize() -> void:
	check(Engine.has_singleton("GDGuard"), "Native singleton not registered")
	if not Engine.has_singleton("GDGuard"):
		quit(1)
		return
	var backend := Engine.get_singleton("GDGuard")
	var fixtures: Array = JSON.parse_string(FileAccess.get_file_as_string("res://native_parity.json"))
	for fixture: Dictionary in fixtures:
		var actual: Array = backend.call("lint", "res://" + fixture.path, fixture.source)
		var keys: Array[String] = []
		for issue: Dictionary in actual:
			for field: String in ["rule", "severity", "message", "help", "line", "column", "end_line", "end_column", "start_byte", "end_byte", "fixes"]:
				check(issue.has(field), "Missing native field: " + field)
			keys.append("%s:%d:%d" % [issue.rule, issue.start_byte, issue.end_byte])
		keys.sort()
		var expected: Array = fixture.keys
		expected.sort()
		check(keys == expected, "Native/library parity: " + fixture.path)
	var source := "var text = '😀漢'\nfunc f(unused):\n\tpass\n"
	var issues: Array = backend.call("lint", "res://native_unicode.gd", source)
	var found := false
	for issue: Dictionary in issues:
		if issue.rule == "unused-argument":
			found = true
			check(issue.line == 2 and issue.column == 8, "Unicode/line-column conversion")
			check(not issue.fixes.is_empty() and issue.fixes[0].safety == "unsafe", "Native fixes/safety")
			var applied: Dictionary = backend.call("apply_edits", source, issue.fixes[0].edits)
			check(not applied.has("error") and applied.output.contains("f(_unused)"), "Native single fix")
	check(found, "Unused argument reported")
	var invalid_edits: Array[Dictionary] = [{"start_byte": 0, "end_byte": source.to_utf8_buffer().size(), "replacement": "func broken(:"}]
	var invalid: Dictionary = backend.call("apply_edits", source, invalid_edits)
	check(invalid.has("error") and invalid.output == source, "Invalid code edit rejected")
	var fixed: Dictionary = backend.call("fix", "res://native_unicode.gd", source, true)
	check(fixed.applied == 1 and fixed.output.contains("f(_unused)"), "Native fix all unsafe")
	var legacy: Array = backend.call("lint_gdscript", "var badName = 1\n", {})
	check(not legacy.is_empty(), "Legacy lint method preserved")
	print("Native integration: %d failures, %d parity fixtures" % [failures, fixtures.size()])
	quit(1 if failures else 0)
