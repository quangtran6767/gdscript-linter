func check(data: ProjectData, engine: EngineData):
	print(data.value)
	print(data.missing)
	print(engine.unknown_engine_member)
	emit_signal("emitted_elsewhere")
	data.connected_in_code.connect(check)
