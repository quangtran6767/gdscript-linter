func clamp_value(value):
	if value < 0:
		return 0
	elif value > 100:
		return 100
	else:
		print("in range")
		return value


func inline(value):
	if value:
		return 1
	else: return 2


func keep_elif(value):
	if value < 0:
		return 0
	elif value > 100:
		print("large")
	elif value > 50:
		print("medium")


func declares(value):
	if value:
		return 1
	else:
		var doubled = value * 2
		return doubled


func multiline_string(value):
	if value:
		return ""
	else:
		return """a
		b"""
