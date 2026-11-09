extends Node2D

const RANGE := 32.0

#region Good

class Ruler:
	func distance_to(_other) -> float:
		return 0.0


func good(a: Vector2, b: Vector2, cell: Vector2i, word: String, ruler: Ruler) -> void:
	print(a.distance_to(b) == 1.0)
	print(a.distance_to(b) / 2.0)
	print(cell.distance_to(Vector2i.ZERO) < 3.0)
	print(word.length() < 3)
	print(ruler.distance_to(a) < 4.0)
	print(a.distance_to(b) < get_radius())
	print(a.distance_to(b) < -1.0)
	print(a.distance_to(b) < 0)
	var unknown = get_vector()
	print(unknown.length() < 3.0)


func get_radius() -> float:
	return 1.0


func get_vector() -> Vector2:
	return Vector2.ZERO

#endregion

#region Bad

func bad(a: Vector2, b: Vector3, c: Vector3, target: Node2D, radius: float) -> void:
	print(a.distance_to(global_position) < 40.0)
	print(b.distance_to(c) <= 40)
	print(40.0 > a.length())
	print((b - c).length() >= radius)
	print(target.global_position.distance_to(position) < RANGE)
	print(a.length() > radius * 2.0)
	var start := Vector2(1, 2)
	print(start.length() < target.scale.x)

#endregion
