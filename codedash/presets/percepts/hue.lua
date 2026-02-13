-- Hue: blue(240)=low -> red(0)=high
local percept = require("codedash.model.percept")
return percept "hue" { range = { 240, 0 } }
