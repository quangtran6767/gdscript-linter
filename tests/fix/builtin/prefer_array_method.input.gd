var nodes: Array[Node] = []


func any_visible() -> bool:
	for node in nodes:
		if node.visible:
			return true
	return false


func all_visible(list: Array) -> bool:
	for item in list:
		if not item.visible:
			return false
	return true


func visible_nodes() -> Array[Node]:
	var result: Array[Node] = []
	for node in nodes:
		if node.visible:
			result.append(node)
	return result


func node_names() -> Array:
	var names := []
	for node in nodes:
		names.append(node.name)
	return names


func copy() -> Array[Node]:
	var result: Array[Node] = []
	for node in nodes:
		result.append(node)
	return result


func all_ready() -> bool:
	for node in nodes:
		if not node.is_node_ready() or node.name == "skip":
			return false
	return true
