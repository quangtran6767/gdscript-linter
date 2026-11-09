#region Good

var one_line: Array[int] = []
var dictionary: Dictionary[String, int] = {}
# Godot accepts a backslash continuation.
var continued: Array[\
	int] = []
var text = "Array[\n]"


# Godot accepts line breaks in parameter types.
func takes(values: Array[
	int
]) -> void:
	print(values)


func callback() -> void:
	var lambda = func(items: Array[
		int
	]): return items
	print(lambda)

#endregion

#region Bad

var broken: Array[
	int
] = []
const TABLE: Dictionary[String,
	int] = {}
var commented: Array[ # Godot rejects this too
	int
] = []


func living() -> Array[
	Node
]:
	for item: Array[
		int
	] in []:
		print(item)
	return []

#endregion
