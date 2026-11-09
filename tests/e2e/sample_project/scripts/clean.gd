## A script with no lint issues.
class_name Clean
extends Node

var _timer := 0.0


func _process(delta: float) -> void:
	_timer += delta
