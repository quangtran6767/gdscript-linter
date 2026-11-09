var items: Array[int] = []
var names := []
var label := ""


func check(list: Array, text: String) -> void:
	print(items.is_empty())
	print(not items.is_empty())
	print(names.is_empty())
	print(label.is_empty())
	print(not text.is_empty())
	if (not items.is_empty()) and not list.is_empty():
		pass
	print((not items.is_empty()) == true)
