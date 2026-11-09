tool
extends Node

onready var label = $Label
export var speed = 10
export(int) var hp = 3
var mana = 5 setget set_mana

remote func sync_position():
	pass


func set_mana(value):
	mana = value


func _ready():
	yield(get_tree(), "idle_frame")
	var enemy = preload("res://enemy.tscn").instance()
	connect("tree_exited", self, "_on_exit")
	label.connect("ready", enemy, "queue_free")
	# Not code: export var, onready var, yield(a, "b")
	print("tool")
	var text = """
export var in_a_string = 1
"""
	connect("ready", _on_exit)
	await get_tree().process_frame


func _on_exit():
	pass
