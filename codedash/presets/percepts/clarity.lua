-- Clarity: dim(0)=low -> bright(1)=high
local percept = require("codedash.model.percept")
return percept "clarity" { range = { 0, 1.0 } }
