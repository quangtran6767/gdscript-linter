class_name Player
extends CharacterBody2D

signal died

const SPEED = 300.0

var inventory: Array = []
var _health := 100


func _process(delta: float) -> void:
	# Looked up every frame: the planned `no-get-node-in-process` rule flags it.
	var health_bar = get_node("HealthBar")
	health_bar.value = _health
	if len(inventory) == 0:
		print("inventory is empty")


func take_damage(amount: int, source: Node) -> void:
	_health -= amount
	if _health <= 0:
		died.emit()


func has_items() -> bool:
	# The planned `prefer-is-empty` rule flags `len(...) > 0` too.
	return len(inventory) > 0
