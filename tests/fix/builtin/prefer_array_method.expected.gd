var nodes: Array[Node] = []


func any_visible() -> bool:
	return nodes.any(
		func(node: Node):
			return node.visible,
	)


func all_visible(list: Array) -> bool:
	return list.all(
		func(item):
			return item.visible,
	)


func visible_nodes() -> Array[Node]:
	var result: Array[Node] = []
	result.assign(
		nodes.filter(
			func(node: Node):
				return node.visible,
		)
	)
	return result


func node_names() -> Array:
	var names := []
	names.assign(
		nodes.map(
			func(node: Node):
				return node.name,
		)
	)
	return names


func copy() -> Array[Node]:
	var result: Array[Node] = []
	result.assign(nodes)
	return result


func all_ready() -> bool:
	return nodes.all(
		func(node: Node):
			return node.is_node_ready() and node.name != "skip",
	)
