#region Good

func early(value):
	if value:
		return 1
	# A comment after return is fine.
	return 2


func loops(items):
	for item in items:
		if item:
			continue
		print(item)

#endregion

#region Bad

func after_return():
	return 1
	print("never")
	print("also never")


func after_break(items):
	for item in items:
		break
		print(item)
	while true:
		continue
		pass


func in_match(value):
	match value:
		1:
			return "one"
			print("never")

#endregion
