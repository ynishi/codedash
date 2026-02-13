# codedash

A Lua DSL for mapping code metrics to visual perceptions.

codedash takes enriched AST data (structural metrics, git history, coverage) and transforms it into visual parameters (color, size, opacity, etc.) through a declarative binding pipeline.

## Install

```bash
luarocks install codedash
```

Or from source:

```bash
luarocks make codedash-scm-1.rockspec
```

## Architecture

```
Index (what to measure)  -->  Normalizer [0,1]  -->  Percept (how to perceive)
     ^                                                     |
     |                                                     v
   Node data                                         Visual output
```

### Core Concepts

| Concept       | Description                                      | Example                         |
|---------------|--------------------------------------------------|---------------------------------|
| **Node**      | Immutable AST-derived data record                | `Node<function start>`          |
| **Index**     | Extracts a numeric metric from a Node            | `index "lines" { source = "lines" }` |
| **Percept**   | Maps normalized [0,1] to a visual range          | `percept "hue" { range = {240, 0} }` |
| **Binding**   | Pairs an Index with a Percept                    | `bind { idx.lines, pct.size }`  |
| **Normalizer**| Converts raw values to [0,1] distribution        | `percentile`, `rank`            |

### Pipeline

1. **Index** resolves a raw numeric value from each Node
2. **Normalizer** computes dataset statistics and maps raw -> [0,1]
3. **Percept** maps [0,1] -> visual value via linear interpolation

## Quick Start

```bash
codedash data.json                          # recommended preset
codedash data.json --config my.lua          # custom config
codedash data.json --top 20                 # top 20 entries
codedash data.json --domain core            # filter by domain
```

### Project Config

Place `.codedash.lua` in your project root:

```lua
-- .codedash.lua
return {
  extends = "recommended",
  domains = {
    { name = "core", patterns = { "router", "route" } },
    { name = "util", patterns = { "utils", "helpers" } },
  },
  exclude = { "index", "vite-env.d" },
}
```

Patterns use **substring matching** against node name, file path, and short name. `"auth"` matches `"auth.ts"`, `"authenticate"`, `"src/auth/login.ts"`, etc.

### Try It Now

```bash
lua examples/basic_run.lua                    -- runs with bundled sample data
lua examples/basic_run.lua your_data.json     -- or bring your own
```

## API Usage

```lua
local codedash = require("codedash")

-- Run with default recommended preset
local instance = codedash.init("enriched.json", { extends = "recommended" })
local report = instance:run()

-- Access results
print(codedash.report.summary(report))
```

## DSL Usage

### Recommended Preset

```lua
-- codedash/presets/recommended.lua
local bind = require("codedash.model.binding")
local idx  = require("codedash.presets.indexes")
local pct  = require("codedash.presets.percepts")

return {
  bindings = {
    bind { idx.churn,    pct.hue },      -- high churn -> red
    bind { idx.lines,    pct.size },     -- many lines -> large
    bind { idx.params,   pct.border },   -- many params -> thick border
    bind { idx.depth,    pct.opacity },  -- deep nesting -> faded
    bind { idx.coverage, pct.clarity },  -- low coverage -> dim
  },
}
```

### Custom Index

```lua
local index = require("codedash.model.index")

-- Direct source (Node field)
local lines = index "lines" { source = "lines" }

-- Computed
local custom = index "risk" {
  compute = function(node) return node.lines * node.cyclomatic end
}

-- Combinator (compose two indexes)
local complexity = index "complexity" {
  combine = { lines_idx, params_idx, function(l, p) return l * 0.3 + p * 2.0 end },
}
```

### Custom Percept

```lua
local percept = require("codedash.model.percept")

local hue = percept "hue" { range = { 240, 0 } }           -- blue(low) -> red(high)
local size = percept "size" { range = { 0.2, 5.0 } }       -- small(low) -> large(high)
local discrete = percept "hue" { range = {240, 0}, steps = 5 }  -- 5-level quantized
```

### Normalizer Override

```lua
-- At Index level (default for all bindings using this index)
local churn = index "churn" { source = "git_churn_30d", normalize = "rank" }

-- At Binding level (overrides index default)
bind { idx.churn, pct.hue, normalize = "rank" }
```

## Directory Structure

```
codedash/
  codedash/         -- Lua modules (installed by LuaRocks)
    init.lua        -- Public API entry point
    model/          -- Core types: Node, Index, Percept, Binding, Normalizer
    eval/           -- Pipeline: loader, settings, classifier, evaluator, report
    lib/            -- Low-level utilities: Range
    presets/        -- Built-in indexes, percepts, normalizers, recommended preset
  tools/            -- AST parser & enricher (Node.js)
    parse_tsx.js    -- tree-sitter TS/TSX parser -> ast_data.json
    enrich_generic.js -- git churn/co-change enricher -> enriched.json
  examples/         -- Usage examples + sample data
  cli.lua           -- CLI runner (installed as `codedash` command)
  codedash-scm-1.rockspec
```

## Generating Input Data

codedash includes tools to generate `enriched.json` from TypeScript/TSX source code.

### Setup

```bash
cd tools && npm install
```

### Usage

```bash
# 1. Parse: TS/TSX files → ast_data.json
node tools/parse_tsx.js src/**/*.ts src/**/*.tsx > ast_data.json

# With --root for monorepo (strips prefix from file names)
node tools/parse_tsx.js --root=packages/app/src packages/app/src/**/*.ts > ast_data.json

# 2. Enrich: ast_data.json + git history → enriched.json
node tools/enrich_generic.js ast_data.json /path/to/repo 'src/**/*.ts' --strip=src/ > enriched.json

# 3. Run codedash
lua examples/basic_run.lua enriched.json
```

### enrich_generic.js Options

| Option | Default | Description |
|--------|---------|-------------|
| `--strip=PREFIX` | — | Strip prefix from git paths to match ast_data names |
| `--churn-days=N` | 30 | Git churn period (days) |
| `--cochange-days=N` | 90 | Co-change detection period (days) |
| `--min-cc=N` | 2 | Minimum co-change count to include |

If the target is not a git repository, churn defaults to 0 and coverage to null (no error).

## Input JSON Format

codedash expects an enriched JSON file with the following structure:

```json
{
  "files": [
    {
      "path": "src/auth.ts",
      "name": "auth",
      "nodes": [
        {
          "kind": "function",
          "name": "login",
          "lines": 25,
          "start_line": 10,
          "end_line": 34,
          "depth": 2,
          "params": 2,
          "field_count": 0,
          "exported": true,
          "visibility": "public",
          "git_churn_30d": 12,
          "coverage": 0.85
        }
      ]
    }
  ]
}
```

### Node Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `kind` | string | yes | — | Semantic type (`"function"`, `"class"`, `"method"`, `"interface"`, etc.) |
| `name` | string | yes | — | Identifier name |
| `lines` | number | no | 1 | Line count |
| `start_line` | number | no | 0 | Start line in file |
| `end_line` | number | no | 0 | End line in file |
| `depth` | number | no | 0 | Nesting depth |
| `params` | number | no | 0 | Parameter count |
| `field_count` | number | no | 0 | Number of fields (for classes/interfaces) |
| `exported` | boolean | no | false | Whether the symbol is exported |
| `visibility` | string | no | `"private"` | Visibility level |
| `cyclomatic` | number | no | *estimated* | Cyclomatic complexity. If omitted, estimated as `lines/10 + params*0.5` |
| `git_churn_30d` | number | no | 0 | Git change count in last 30 days |
| `coverage` | number/null | no | null | Test coverage ratio (0.0-1.0) |

### File Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | yes | File path (e.g. `"src/auth.ts"`) |
| `name` | string | yes | Short identifier for domain matching |
| `nodes` | array | yes | Array of node objects |

## Normalizers

| Name         | Strategy                              | Best for                  |
|--------------|---------------------------------------|---------------------------|
| `percentile` | p10/p90 range, linear interpolation   | Normally distributed data |
| `rank`       | Rank-based, average ties              | Skewed distributions      |

## Requirements

- Lua 5.1+ (or LuaJIT)
- lua-cjson
- Node.js 18+ (for `tools/` AST parser & enricher)
