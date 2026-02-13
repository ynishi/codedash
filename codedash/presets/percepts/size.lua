-- Node size: small=low -> large=high
local percept = require("codedash.model.percept")
return percept "size" { range = { 0.2, 5.0 } }
