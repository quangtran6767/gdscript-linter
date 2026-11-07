func _ready():
	var items = []
	if len(items) == 0:
		print("empty", 1)
	var p = get_node("Player")

func _process(delta):
	get_node("Enemy").hide()
