#region Good

var nodes: Array[Node] = []


func with_index() -> bool:
	for index in range(nodes.size()):
		if nodes[index].visible:
			return true
	return false


func with_comment() -> bool:
	for node in nodes:
		# A comment would be lost.
		if node.visible:
			return true
	return false


func with_else() -> bool:
	for node in nodes:
		if node.visible:
			return true
		else:
			print(node)
	return false


func same_result() -> bool:
	for node in nodes:
		if node.visible:
			return true
	return true


func with_await() -> bool:
	for node in nodes:
		if await node.ready:
			return true
	return false


func unknown_source(items) -> Array:
	var result := []
	for item in items:
		result.append(item)
	return result


func filter_and_map() -> Array:
	var result := []
	for node in nodes:
		if node.visible:
			result.append(node.name)
	return result


func not_empty_first() -> Array:
	var result := [1]
	for node in nodes:
		result.append(node)
	return result


func uses_result() -> Array:
	var result := []
	for node in nodes:
		result.append(result.size())
	return result

#endregion

#region Bad

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


func none_ready() -> bool:
	for node: Node in nodes:
		if node.is_node_ready() and node.visible:
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

#endregion
