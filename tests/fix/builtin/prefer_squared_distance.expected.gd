extends Node2D

const RANGE := 32.0


func check(a: Vector2, b: Vector3, c: Vector3, target: Node2D, radius: float) -> void:
	print(a.distance_squared_to(global_position) < 40.0 * 40.0)
	print(b.distance_squared_to(c) <= 1600)
	print(40.0 * 40.0 > a.length_squared())
	print((b - c).length_squared() >= radius * radius)
	print(target.global_position.distance_squared_to(position) < RANGE * RANGE)
	print(a.length_squared() > (radius * 2.0) * (radius * 2.0))
