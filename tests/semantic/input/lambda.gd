func f():
	var captured = 1
	var callback = func(extra):
		return captured + extra
	return callback
