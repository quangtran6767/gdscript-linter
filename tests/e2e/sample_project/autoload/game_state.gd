extends Node

signal score_changed(new_score: int)

const MAX_LIVES = 3

var score := 0
var lives := MAX_LIVES


func add_points(points: int) -> void:
	score += points
	score_changed.emit(score)


func reset() -> void:
	score = 0
	lives = MAX_LIVES
