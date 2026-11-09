signal changed(value: int)
func f():
	changed.emit(1)
	changed.connect(f)
