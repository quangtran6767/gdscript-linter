# gdguard-ignore-file enum-name -- legacy names
enum state { IDLE }
enum other_state { IDLE }

const bad_const = 1 # gdguard-ignore constant-name -- kept for the save format

# gdguard-ignore-next-line constant-name, signal-name
const another_bad = 2

const fine = 3 # gdguard-ignore-line constant-name

var clean = 4 # gdguard-ignore

# gdlint-ignore-next-line private-access
var also_clean = 5
