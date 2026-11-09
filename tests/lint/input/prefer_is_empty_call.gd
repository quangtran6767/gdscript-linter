#region Good

class Queue:
	func size() -> int:
		return 0


var queue := Queue.new()
var items: Array[int] = []


func good(unknown, count: int) -> void:
	print(queue.size() > 0)
	print(unknown.size() > 0)
	print(items.size() == 1)
	print(items.size() > 2)
	print(count > 0)
	print(items.size() >= 0)
	var inferred := get_items()
	print(inferred.size() > 0)


func get_items() -> Array:
	return items

#endregion

#region Bad

var names := []
var table := {}
var label := ""
var packed: PackedStringArray


func bad(list: Array, text: String) -> void:
	print(items.size() == 0)
	print(items.size() <= 0)
	print(items.size() < 1)
	print(items.size() != 0)
	print(items.size() > 0)
	print(items.size() >= 1)
	print(0 < items.size())
	print(len(names) == 0)
	print(table.size() > 0)
	print(label.length() == 0)
	print(text.length() > 0)
	print(len(list) != 0)
	print(packed.size() == 0)
	if (items.size() > 0) and names.size() == 0:
		pass
	print((items.size() > 0) == true)
	print(items.size() > 0 == true)

#endregion
