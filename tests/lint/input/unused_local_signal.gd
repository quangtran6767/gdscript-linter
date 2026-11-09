signal unused
signal emitted
signal connected

func f():
	emitted.emit()
	connect("connected", f)
