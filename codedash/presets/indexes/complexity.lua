-- Computed Index: complexity from lines + params (Combinator)
local index = require("codedash.model.index")
return index "complexity" {
  combine = {
    require("codedash.presets.indexes.lines"),
    require("codedash.presets.indexes.params"),
    function(lines, params) return lines * 0.3 + params * 2.0 end,
  },
}
