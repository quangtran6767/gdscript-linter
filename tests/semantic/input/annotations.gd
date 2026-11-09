@export var speed: float = 1.0
@onready var sprite: Sprite2D = $Sprite
func f():
	sprite.position.x = speed
