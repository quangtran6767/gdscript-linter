extends Node2D

const RANGE := 32.0


func check(a: Vector2, b: Vector3, c: Vector3, target: Node2D, radius: float) -> void:
	print(a.distance_to(global_position) < 40.0)
	print(b.distance_to(c) <= 40)
	print(40.0 > a.length())
	print((b - c).length() >= radius)
	print(target.global_position.distance_to(position) < RANGE)
	print(a.length() > radius * 2.0)
