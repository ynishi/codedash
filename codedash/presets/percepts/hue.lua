-- Hue: green(120)=low -> red(0)=high (traffic-light gradient)
local percept = require("codedash.model.percept")
return percept "hue" { range = { 120, 0 } }
