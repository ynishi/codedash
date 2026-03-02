# CI Integration Guide

Automate code metrics with GitHub Actions. Two workflows are provided:

| Workflow | Trigger | Output |
|----------|---------|--------|
| **Metrics Dashboard** | `push` to `main` | GitHub Pages site with module map, coverage, benchmarks |
| **PR Metrics Comment** | Pull request | Sticky comment with metrics summary table |

## Prerequisites

The workflows install `cargo-llvm-cov` and `codedash` automatically via `cargo install`.
codedash のネイティブ依存（tree-sitter, libgit2, Lua 5.4）は全て vendored されているため、`apt-get install` 等の追加パッケージは不要です。

You need to prepare:

| Item | Why | How |
|------|-----|-----|
| **Tests** | `cargo llvm-cov` runs your test suite to measure coverage. No tests = empty report | `cargo test` が通ること |
| **`.codedash.lua`** | codedash の設定ファイル。リポジトリにコミットされている必要がある | `codedash config-init` で生成 → `git add` |
| **criterion benchmarks** (optional) | `cargo bench` ステップで使用。不要なら削除可 | `Cargo.toml` に `[dev-dependencies] criterion = ...` + `[[bench]]` ターゲット |
| **GitHub Pages** | ダッシュボードのホスティング先 | Settings → Pages → Source: **GitHub Actions** |

## 1. Project Configuration

Generate `.codedash.lua` in your project root and commit it:

```bash
codedash config-init
git add .codedash.lua
```

For Cargo workspaces, `config-init` auto-detects crate members and generates domain/layer definitions.

## 2. Metrics Dashboard (GitHub Pages)

Create `.github/workflows/metrics.yml`:

```yaml
name: Metrics

on:
  push:
    branches: [main]

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  build:
    name: Build Reports
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          components: llvm-tools-preview

      - uses: Swatinem/rust-cache@v2

      - name: Install cargo-llvm-cov
        uses: taiki-e/install-action@cargo-llvm-cov

      - name: Install codedash
        run: cargo install codedash --locked

      # Coverage
      - name: Generate coverage
        run: |
          cargo llvm-cov --workspace --json --output-path coverage.json
          cargo llvm-cov --workspace --html --output-dir coverage-html

      # Codedash reports
      - name: Generate codedash view
        run: codedash view . --cov-file coverage.json -o codedash-view.html

      - name: Generate codedash JSON
        run: codedash analyze . --cov-file coverage.json -o json > codedash-metrics.json

      # Benchmarks (optional — remove if not using criterion)
      - name: Run benchmarks
        run: cargo bench --workspace

      # Assemble site
      - name: Assemble GitHub Pages site
        run: |
          mkdir -p _site/coverage _site/benchmarks

          cp codedash-view.html _site/
          cp -r coverage-html/html/* _site/coverage/
          if [ -d target/criterion/report ]; then
            cp -r target/criterion/report/* _site/benchmarks/
          fi
          cp codedash-metrics.json _site/
          cp coverage.json _site/

          cat > _site/index.html <<'INDEXEOF'
          <!DOCTYPE html>
          <html lang="en">
          <head>
            <meta charset="utf-8">
            <title>Metrics Dashboard</title>
            <style>
              :root{--bg:#0d1117;--bg2:#161b22;--fg:#c9d1d9;--fg2:#8b949e;--border:#30363d;--accent:#58a6ff}
              body{background:var(--bg);color:var(--fg);font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,sans-serif;max-width:720px;margin:40px auto;padding:0 20px}
              h1{font-size:24px;margin-bottom:8px}
              .sub{color:var(--fg2);margin-bottom:32px;font-size:14px}
              a{color:var(--accent);text-decoration:none}
              a:hover{text-decoration:underline}
              .card{background:var(--bg2);border:1px solid var(--border);border-radius:8px;padding:20px;margin:12px 0;display:flex;justify-content:space-between;align-items:center}
              .card h2{font-size:16px;margin:0}
              .card p{color:var(--fg2);font-size:13px;margin:4px 0 0}
              .arrow{color:var(--fg2);font-size:20px}
            </style>
          </head>
          <body>
            <h1>Metrics Dashboard</h1>
            <div class="sub">Updated on each push to main</div>
            <a href="codedash-view.html">
              <div class="card"><div><h2>Module Map</h2><p>Interactive dependency graph with coverage overlay</p></div><span class="arrow">&rarr;</span></div>
            </a>
            <a href="coverage/index.html">
              <div class="card"><div><h2>Coverage Report</h2><p>Line-level coverage from cargo-llvm-cov</p></div><span class="arrow">&rarr;</span></div>
            </a>
            <a href="benchmarks/index.html">
              <div class="card"><div><h2>Benchmarks</h2><p>Criterion performance reports</p></div><span class="arrow">&rarr;</span></div>
            </a>
            <a href="codedash-metrics.json">
              <div class="card"><div><h2>Metrics JSON</h2><p>Raw codedash analysis data</p></div><span class="arrow">&rarr;</span></div>
            </a>
          </body>
          </html>
          INDEXEOF

      - name: Upload Pages artifact
        uses: actions/upload-pages-artifact@v3
        with:
          path: _site

  deploy:
    name: Deploy
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - name: Deploy to GitHub Pages
        id: deployment
        uses: actions/deploy-pages@v4
```

### Enable GitHub Pages

Go to **Settings > Pages > Source** and select **GitHub Actions**.

After the next push to `main`, the dashboard will be available at:

```
https://<owner>.github.io/<repo>/
```

### Deployed site structure

```
/
├── index.html              ← Dashboard with links to all reports
├── codedash-view.html      ← Interactive module map with coverage overlay
├── codedash-metrics.json   ← Raw metrics JSON
├── coverage.json           ← llvm-cov raw data
├── coverage/index.html     ← Line-level coverage HTML
└── benchmarks/index.html   ← Criterion HTML report
```

## 3. PR Metrics Comment

Create `.github/workflows/pr-metrics.yml`:

```yaml
name: PR Metrics

on:
  pull_request:
    branches: [main]

permissions:
  pull-requests: write

jobs:
  comment:
    name: Metrics Comment
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          components: llvm-tools-preview

      - uses: Swatinem/rust-cache@v2

      - name: Install cargo-llvm-cov
        uses: taiki-e/install-action@cargo-llvm-cov

      - name: Install codedash
        run: cargo install codedash --locked

      - name: Generate metrics
        run: |
          cargo llvm-cov --workspace --json --output-path coverage.json
          codedash analyze . --cov-file coverage.json -o json > metrics.json

      - name: Build comment body
        id: body
        run: |
          BODY=$(python3 <<'PYEOF'
          import json

          cov = json.load(open("coverage.json"))
          met = json.load(open("metrics.json"))

          totals = cov["data"][0]["totals"]
          line_pct = totals["lines"]["percent"]
          fn_pct = totals["functions"]["percent"]
          region_pct = totals["regions"]["percent"]

          entries = met["entries"]
          total = met["total"]
          groups = met["groups"]

          high_cyclo = [e for e in entries if e.get("cyclomatic", 0) >= 15]
          large_fn = [e for e in entries if e.get("lines", 0) >= 100]

          lines = []
          lines.append("## Code Metrics")
          lines.append("")
          lines.append("| Metric | Value |")
          lines.append("|---|---|")
          lines.append(f"| Code units | {total} |")
          lines.append(f"| Line coverage | {line_pct:.1f}% |")
          lines.append(f"| Function coverage | {fn_pct:.1f}% |")
          lines.append(f"| Region coverage | {region_pct:.1f}% |")
          lines.append(f"| High complexity (cyclo>=15) | {len(high_cyclo)} |")
          lines.append(f"| Large functions (>=100 lines) | {len(large_fn)} |")
          lines.append("")

          lines.append("<details><summary>Domain breakdown</summary>")
          lines.append("")
          lines.append("| Domain | Nodes | % |")
          lines.append("|---|---|---|")
          for g in sorted(groups, key=lambda x: x.get("count", 0), reverse=True):
              pct = g["count"] / total * 100 if total else 0
              lines.append(f"| {g['name']} | {g['count']} | {pct:.1f}% |")
          lines.append("")
          lines.append("</details>")

          print("\n".join(lines))
          PYEOF
          )

          echo "$BODY" > /tmp/pr-comment.md

      - name: Post comment
        uses: marocchino/sticky-pull-request-comment@v2
        with:
          header: codedash-metrics
          path: /tmp/pr-comment.md
```

### PR comment output example

The workflow posts a table like:

| Metric | Value |
|---|---|
| Code units | 142 |
| Line coverage | 78.3% |
| Function coverage | 65.1% |
| Region coverage | 72.4% |
| High complexity (cyclo>=15) | 3 |
| Large functions (>=100 lines) | 5 |

With a collapsible domain breakdown section.

## Customization

### Without benchmarks

Remove the `Run benchmarks` step and `_site/benchmarks` directory from `metrics.yml`.

### Without coverage

Remove `cargo-llvm-cov` steps and `--cov-file` flags. codedash works without coverage data — the coverage overlay will simply be absent.

### Complexity/size thresholds

Adjust the Python filters in `pr-metrics.yml`:

```python
# Change thresholds to match your standards
high_cyclo = [e for e in entries if e.get("cyclomatic", 0) >= 10]  # stricter
large_fn = [e for e in entries if e.get("lines", 0) >= 50]         # stricter
```
