-- Opacity: opaque(1.0)=low -> transparent(0.1)=high
local percept = require("codedash.model.percept")
return percept "opacity" { range = { 1.0, 0.1 } }
