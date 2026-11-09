class_name badClass
extends Node

signal BadSignal

enum state { idle_state }

const maxSpeed = 10
const icon_scene = preload("res://icon.svg")

var badVariable = 1
var Bad_Texture = load("res://icon.svg")


func badName(BadArg, unused_arg: int) -> void:
	if BadArg == BadArg:
		pass
	1 + 2
	pass
	for Item in range(3):
		print(Item)
	var other = Node.new()
	print(other._secret)
	other._hidden_method()


func pick(value: int) -> int:
	if value > 0:
		return 1
	elif value < 0:
		return -1
	else:
		return 0


# The é characters are two bytes each: the span starts at a byte column.
var long_line = "éééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé"


func _unused_helper():
	var never_used = 1
	var assigned = 0
	assigned = 2
	var badVariable = 0
	print(badVariable)


func project_member(data: ProjectData):
	return data.missing


func newer_rules(value):
	value = value
	var preloaded = preload("res://data.gd")
	var missing_icon = load("res://missing_icon.png")
	var stats = {"hp": 1, "hp": 2}
	yield(get_tree(), "idle_frame")
	return [preloaded, missing_icon, stats]
	print("unreachable")


var broken_type: Array[
	int
] = []


func w11_rules(items: Array[Node], a: Vector2, b: Vector2) -> bool:
	if items.size() > 0:
		print(a.distance_to(b) < 4.0)
	for item in items:
		if item.visible:
			return true
	return false
