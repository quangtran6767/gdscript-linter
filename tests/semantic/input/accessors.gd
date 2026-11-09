var value: int = 0:
	set(next):
		value = next
	get:
		return value

func _init(start: int):
	value = start
