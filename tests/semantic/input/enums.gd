enum State { IDLE, RUN = 2 }
const DEFAULT = 1
func f():
	return State.IDLE + DEFAULT
