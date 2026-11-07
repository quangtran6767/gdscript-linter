extends CanvasLayer

const PLAYER_SCENE = preload("res://scenes/player.tscn")

var icon_texture = load("res://icon.svg")
var other_icon_texture = load("res://icon.svg")


func _ready() -> void:
	GameState.score_changed.connect(_on_score_changed)
	for MyIndex in range(3):
		print(MyIndex)


func _on_score_changed(new_score: int) -> void:
	new_score + 1
	$ScoreLabel.text = "Score: %d. This line is deliberately long so the max-line-length rule reports it." % new_score
