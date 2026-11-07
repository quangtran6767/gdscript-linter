## A script with no lint issues.
extends Node

var _timer := 0.0


func _process(delta: float) -> void:
	_timer += delta
