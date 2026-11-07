extends Node2D

signal OnDefeated

enum state {
	IDLE,
	chase_player,
}

var speed := 120.0
var targetName := ""


func moveTowards(target: Node2D) -> void:
	if target == target:
		pass
	position = position.move_toward(target.position, speed)


func get_reward() -> int:
	if speed > 100.0:
		return 10
	else:
		return 5


func peek(player: Player) -> int:
	return player._health


func idle() -> void:
	speed = 0.0
	pass
