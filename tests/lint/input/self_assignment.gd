#region Good

var speed = 0:
	set(value):
		speed = value


func refresh():
	# Re-running a member setter is a Godot idiom.
	speed = speed
	self.speed = self.speed


func copy(value):
	var other = value
	value = other
	return value

#endregion

#region Bad

func set_speed(speed_value):
	speed_value = speed_value


func loop(items):
	for item in items:
		item = item
	var total = 0
	total = total
	return total

#endregion
