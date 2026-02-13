-- Weight: light=low -> heavy=high
local percept = require("codedash.model.percept")
return percept "weight" { range = { 0.2, 5.0 } }
