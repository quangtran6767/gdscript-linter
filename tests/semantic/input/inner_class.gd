var outer_member = 1
const SHARED = 2
class Inner:
	var own = 3
	func f():
		return outer_member + SHARED + own
