// enrich_generic.js - ast_data.json にgit実データを付与（汎用版）
//
// Usage: node tools/enrich_generic.js <ast_data.json> <repo_path> <glob1> [glob2...] [--options]
//
// Examples:
//   node enrich_generic.js ast.json ./repo 'src/GLOB' --strip=src/
//   node enrich_generic.js ast.json ./repo 'packages/editor/src/GLOB' --strip=packages/editor/src/
//
// Options:
//   --strip=PREFIX       Strip this prefix from file paths
//   --churn-days=N       Churn period (default: 30)
//   --cochange-days=N    Co-change period (default: 90)
//   --min-cc=N           Minimum co-change count (default: 2)

const fs = require("fs");
const { execSync } = require("child_process");
const path = require("path");

// Parse arguments
const args = process.argv.slice(2);
const flags = {};
const positional = [];

for (const arg of args) {
  if (arg.startsWith("--")) {
    const [key, val] = arg.slice(2).split("=");
    flags[key] = val || "true";
  } else {
    positional.push(arg);
  }
}

const inputPath = positional[0];
const repoPath = positional[1];
const srcGlobs = positional.slice(2);

if (!inputPath || !repoPath || srcGlobs.length === 0) {
  console.error(`Usage: node enrich_generic.js <ast_data.json> <repo_path> <glob1> [glob2...] [--strip=prefix]

Examples:
  node enrich_generic.js ast.json ./repo 'src/**/*.ts' 'src/**/*.tsx' --strip=src/
  node enrich_generic.js ast.json ./repo 'packages/editor/src/**/*.ts' --strip=packages/editor/src/`);
  process.exit(1);
}

const stripPrefix = flags.strip || "";
const churnDays = parseInt(flags["churn-days"] || "30", 10);
const cochangeDays = parseInt(flags["cochange-days"] || "90", 10);
const minCc = parseInt(flags["min-cc"] || "2", 10);

const data = JSON.parse(fs.readFileSync(inputPath, "utf-8"));

// Build git glob arguments
const gitGlobs = srcGlobs.map(g => `'${g}'`).join(" ");

// ============================================================
// Normalize file path to match ast_data.json names
// ============================================================
function normalizePath(filePath) {
  let clean = filePath.trim();
  // Remove source file extensions (.ts/.tsx/.rs)
  clean = clean.replace(/\.(tsx?|rs)$/, "");
  // git log may output paths relative to repo root with subdir prefix
  // e.g., "orcs-desktop/src/foo.ts" when repo has subdir structure
  // Try to find and strip the configured prefix anywhere in the path
  if (stripPrefix) {
    const idx = clean.indexOf(stripPrefix);
    if (idx >= 0) {
      clean = clean.slice(idx + stripPrefix.length);
    }
  }
  return clean;
}

// ============================================================
// Git Churn (configurable period, file level)
// ============================================================
console.error(`[enrich] Fetching git churn (${churnDays}d) from ${repoPath}...`);

let churnOutput;
try {
  churnOutput = execSync(
    `cd "${repoPath}" && git log --since="${churnDays} days ago" --name-only --pretty=format: -- ${gitGlobs}`,
    { encoding: "utf-8", maxBuffer: 50 * 1024 * 1024 }
  );
} catch (e) {
  console.error(`[enrich] WARN: git churn failed: ${e.message}`);
  churnOutput = "";
}

const churn = {};
for (const line of churnOutput.split("\n")) {
  const trimmed = line.trim();
  if (trimmed) {
    const clean = normalizePath(trimmed);
    if (clean) {
      churn[clean] = (churn[clean] || 0) + 1;
    }
  }
}

console.error(`[enrich] Churn: ${Object.keys(churn).length} files with changes`);

// ============================================================
// Co-change (configurable period, file level)
// ============================================================
console.error(`[enrich] Fetching co-change (${cochangeDays}d) from ${repoPath}...`);

let cochangeOutput;
try {
  cochangeOutput = execSync(
    `cd "${repoPath}" && git log --since="${cochangeDays} days ago" --name-only --pretty=format:"---COMMIT---" -- ${gitGlobs}`,
    { encoding: "utf-8", maxBuffer: 50 * 1024 * 1024 }
  );
} catch (e) {
  console.error(`[enrich] WARN: git co-change failed: ${e.message}`);
  cochangeOutput = "";
}

const commits = [];
let current = [];
for (const line of cochangeOutput.split("\n")) {
  if (line === "---COMMIT---") {
    if (current.length > 0) commits.push(current);
    current = [];
  } else {
    const trimmed = line.trim();
    if (trimmed) {
      const clean = normalizePath(trimmed);
      if (clean) {
        current.push(clean);
      }
    }
  }
}
if (current.length > 0) commits.push(current);

const coChangePairs = {};
for (const commit of commits) {
  const uniq = [...new Set(commit)];
  for (let i = 0; i < uniq.length; i++) {
    for (let j = i + 1; j < uniq.length; j++) {
      const a = uniq[i], b = uniq[j];
      const key = a < b ? `${a}|${b}` : `${b}|${a}`;
      coChangePairs[key] = (coChangePairs[key] || 0) + 1;
    }
  }
}

const ccAboveMin = Object.values(coChangePairs).filter(v => v >= minCc).length;
console.error(`[enrich] Co-change: ${commits.length} commits, ${Object.keys(coChangePairs).length} pairs total, ${ccAboveMin} pairs with cc>=${minCc}`);

// ============================================================
// Match diagnostics: check how many ast_data files match churn
// ============================================================
let matchedFiles = 0;
let unmatchedFiles = [];
for (const file of data.files) {
  if (churn[file.name] !== undefined) {
    matchedFiles++;
  } else {
    unmatchedFiles.push(file.name);
  }
}
console.error(`[enrich] File matching: ${matchedFiles}/${data.files.length} files matched churn data`);
if (unmatchedFiles.length > 0 && unmatchedFiles.length <= 10) {
  console.error(`[enrich] Unmatched: ${unmatchedFiles.join(", ")}`);
} else if (unmatchedFiles.length > 10) {
  console.error(`[enrich] Unmatched: ${unmatchedFiles.slice(0, 10).join(", ")} ... and ${unmatchedFiles.length - 10} more`);
}

// ============================================================
// Enrich
// ============================================================
for (const file of data.files) {
  file.git_churn_30d = churn[file.name] || 0;

  for (const node of file.nodes) {
    node.git_churn_30d = churn[file.name] || 0;
    node.co_changes = {};
  }
}

// co-changeをファイル代表ノード間で注入
const callableKinds = new Set(["function", "component", "hook", "method", "store", "context", "macro"]);
function getRepresentatives(fileName) {
  const file = data.files.find(f => f.name === fileName);
  if (!file) return [];
  const callables = file.nodes.filter(n => callableKinds.has(n.kind));
  const exported = callables.filter(n => n.exported);
  return exported.length > 0 ? exported : callables.slice(0, 3);
}

for (const [key, count] of Object.entries(coChangePairs)) {
  if (count < minCc) continue;

  const [fileA, fileB] = key.split("|");
  const repsA = getRepresentatives(fileA);
  const repsB = getRepresentatives(fileB);

  for (const nodeA of repsA) {
    for (const nodeB of repsB) {
      nodeA.co_changes[`${fileB}::${nodeB.name}`] = count;
      nodeB.co_changes[`${fileA}::${nodeA.name}`] = count;
    }
  }
}

// coverage: null for all (no real data available)
for (const file of data.files) {
  for (const node of file.nodes) {
    node.coverage = null;
  }
}

// Summary
let totalCoChanges = 0;
for (const file of data.files) {
  for (const node of file.nodes) {
    totalCoChanges += Object.keys(node.co_changes).length;
  }
}
console.error(`[enrich] Result: ${data.files.length} files, ${totalCoChanges} co-change entries injected`);

console.log(JSON.stringify(data, null, 2));
