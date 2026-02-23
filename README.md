# codedash

Code metrics visualization CLI — parse, enrich, and map code structure to visual perceptions.

codedash analyzes source code via tree-sitter AST parsing, enriches it with git history, and transforms structural metrics (lines, complexity, churn, depth) into visual parameters (color, size, opacity) through a declarative Lua DSL pipeline.

## Install

```bash
cargo install codedash
```

### From source

```bash
git clone https://github.com/ynishi/codedash.git
cd codedash
cargo install --path .
```

## Quick Start

```bash
# Analyze current directory (Rust)
codedash analyze

# Analyze TypeScript project
codedash analyze src/ -l typescript

# JSON output
codedash analyze -o json

# Top 20 entries
codedash analyze -t 20

# Filter by domain
codedash analyze -d core

# Generate interactive HTML dependency map
codedash view

# With coverage data (cargo-llvm-cov --json output)
codedash analyze --cov-file coverage.json -o json
codedash view --cov-file coverage.json

# Show dependency graph (DOT format)
codedash graph

# Diagnose setup
codedash check-health
```

## Commands

| Command | Description |
|---------|-------------|
| `analyze` | Parse + enrich + evaluate metrics with visual mapping |
| `parse` | Parse source files (AST only, no enrichment) |
| `graph` | Show dependency graph (DOT or Mermaid format) |
| `view` | Generate interactive HTML dependency map |
| `config-init` | Generate `.codedash.lua` template (auto-detects Cargo workspace) |
| `list-parsers` | List supported languages and file extensions |
| `check-health` | Diagnose parser, git, and config status |

## Configuration

Place `.codedash.lua` in your project root:

```lua
return {
  extends = "recommended",
  domains = {
    { name = "core", patterns = { "router", "route" } },
    { name = "util", patterns = { "utils", "helpers" } },
  },
  exclude = { "index", "vite-env.d" },
}
```

Generate a template automatically:

```bash
codedash config-init   # auto-detects Cargo workspace members
```

Patterns use **substring matching** against node name, file path, and short name.

## Architecture

```
Source code
  → tree-sitter AST parsing (Rust)
    → Git enrichment (churn, co-change)
    → Coverage enrichment (--cov-file, llvm-cov JSON)
      → Lua DSL evaluation
        → Visual output (report / JSON / HTML)
```

### Pipeline

```
Index (what to measure)  →  Normalizer [0,1]  →  Percept (how to perceive)
     ^                                                  |
     |                                                  v
   Node data                                      Visual output
```

| Concept | Description |
|---------|-------------|
| **Node** | Immutable AST-derived data record |
| **Index** | Extracts a numeric metric from a Node |
| **Percept** | Maps normalized [0,1] to a visual range |
| **Binding** | Pairs an Index with a Percept |
| **Normalizer** | Converts raw values to [0,1] distribution |

### Built-in Bindings (recommended preset)

| Index | Percept | Meaning |
|-------|---------|---------|
| churn | hue | High churn → red |
| lines | size | Many lines → large |
| params | border | Many params → thick border |
| depth | opacity | Deep nesting → faded |
| coverage | clarity | Low coverage → dim |

### HTML View Encoding (`codedash view`)

| Visual Channel | Data Source | Mapping |
|----------------|-------------|---------|
| Circle size | Lines of code | More lines → larger |
| Circle color | Domain | Auto-assigned palette |
| Inner ring color | Cyclomatic complexity | Green (low) → Red (high) |
| Inner ring width | Max cyclomatic | Higher → thicker |
| Outer ring (amber) | Git churn (30d) | More changes → wider |
| Badge (shield) | Coverage | Green ≥70%, Yellow ≥40%, Red <40% |
| Badge (gray ?) | Coverage N/A | No coverage data for this module |
| Edges | Import dependencies | Arrows between modules |

## Customizing the DSL

### Custom Index

```lua
local index = require("codedash.model.index")

local lines = index "lines" { source = "lines" }

local risk = index "risk" {
  compute = function(node) return node.lines * node.cyclomatic end
}
```

### Custom Percept

```lua
local percept = require("codedash.model.percept")

local hue  = percept "hue"  { range = { 240, 0 } }       -- blue → red
local size = percept "size" { range = { 0.2, 5.0 } }     -- small → large
```

### Normalizer Override

```lua
-- At Binding level
bind { idx.churn, pct.hue, normalize = "rank" }
```

| Normalizer | Strategy | Best for |
|------------|----------|----------|
| `percentile` | p10/p90 range, linear interpolation | Normally distributed data |
| `rank` | Rank-based, average ties | Skewed distributions |

## Supported Languages

| Language | Extensions |
|----------|------------|
| Rust | `.rs` |
| TypeScript | `.ts`, `.tsx` |

## Requirements

- Rust 1.80+

## License

MIT
