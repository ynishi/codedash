-- Presence: subtle(0)=low -> prominent(1)=high
local percept = require("codedash.model.percept")
return percept "presence" { range = { 0, 1.0 } }
