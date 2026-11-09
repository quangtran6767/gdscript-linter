#region Good

const KEY = "a"

var distinct = {"a": 1, "b": 2, 1: 3, 1.0: 4}
var styles = {a = 1, "b": 2}
var calls = {randi(): 1, randi(): 2}
var nested = {"a": {"a": 1}}

#endregion

#region Bad

var strings = {"hp": 1, 'hp': 2}
var lua = {hp = 1, mp = 2, hp = 3}
var constants = {KEY: 1, KEY: 2}
var numbers = {
	1: "one",
	2: "two",
	1: "uno",
}

#endregion
