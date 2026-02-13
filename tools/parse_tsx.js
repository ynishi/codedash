/**
 * parse_tsx.js - TS/TSXファイルをtree-sitterで解析 → JSON出力
 *
 * parse_ts.jsのTSX対応版。React特有のパターンも抽出:
 * - Arrow function components (const Foo = () => { ... })
 * - Custom hooks (useXxx)
 * - JSX依存 (他コンポーネントの使用)
 *
 * Usage: node tools/parse_tsx.js file1.ts file2.tsx ... > ast_data.json
 */

const Parser = require("tree-sitter");
const TSLang = require("tree-sitter-typescript").typescript;
const TSXLang = require("tree-sitter-typescript").tsx;
const fs = require("fs");
const path = require("path");

const tsParser = new Parser();
tsParser.setLanguage(TSLang);

const tsxParser = new Parser();
tsxParser.setLanguage(TSXLang);

// Parse --root option: specifies the prefix to strip for relative names
// Usage: node parse_tsx.js --root=/path/to/packages/excalidraw file1.ts file2.tsx ...
let rootDir = null;
const files = [];
for (const arg of process.argv.slice(2)) {
  if (arg.startsWith("--root=")) {
    rootDir = arg.slice(7);
    // Ensure trailing slash
    if (!rootDir.endsWith("/")) rootDir += "/";
  } else {
    files.push(arg);
  }
}
if (files.length === 0) {
  console.error("Usage: node parse_tsx.js [--root=<dir>] <file1.ts> [file2.tsx ...]");
  process.exit(1);
}

const result = {
  files: [],
  edges: [],
};

for (const filePath of files) {
  const source = fs.readFileSync(filePath, "utf-8");
  const ext = path.extname(filePath);
  const parser = ext === ".tsx" ? tsxParser : tsParser;

  let tree;
  try {
    tree = parser.parse(source);
  } catch (e) {
    // 大きなファイルはコールバック方式で再試行
    try {
      tree = parser.parse((index) => {
        if (index >= source.length) return null;
        return source.slice(index, Math.min(index + 10240, source.length));
      });
      console.error(`[INFO] parsed ${filePath} via callback (${source.length} bytes)`);
    } catch (e2) {
      console.error(`[WARN] tree-sitter parse failed for ${filePath} (${source.length} bytes), skipping`);
      continue;
    }
  }

  const baseName = path.basename(filePath, ext);
  // ディレクトリ構造を含めた短縮名
  let relPath;
  if (rootDir) {
    const absFile = path.resolve(filePath);
    const absRoot = path.resolve(rootDir);
    relPath = absFile.startsWith(absRoot) ? absFile.slice(absRoot.length) : absFile;
    relPath = relPath.replace(/\.(tsx?)$/, "");
    // Strip leading slash
    if (relPath.startsWith("/")) relPath = relPath.slice(1);
  } else {
    relPath = filePath.replace(/.*\/src\//, "").replace(ext, "");
  }

  const fileData = {
    path: filePath,
    name: relPath,
    nodes: [],
    imports: [],
  };

  function visit(node, depth) {
    const entries = extractNode(node, source, depth);
    if (entries) {
      for (const entry of Array.isArray(entries) ? entries : [entries]) {
        fileData.nodes.push(entry);
      }
    }

    for (let i = 0; i < node.childCount; i++) {
      visit(node.child(i), depth + 1);
    }
  }

  function extractNode(node, source, depth) {
    const type = node.type;

    // import文
    if (type === "import_statement") {
      const importInfo = extractImport(node, source);
      if (importInfo) fileData.imports.push(importInfo);
      return null;
    }

    // 関数宣言
    if (type === "function_declaration") {
      return {
        kind: "function",
        name: getChildText(node, "name", source) || "(anonymous)",
        exported: isExported(node),
        async: isAsync(node),
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        params: countParams(node),
        depth: depth,
        body_text: node.text,
      };
    }

    // メソッド定義
    if (type === "method_definition" || type === "public_field_definition") {
      const accessibility = getAccessibility(node);
      return {
        kind: "method",
        name: getChildText(node, "name", source) || "(anonymous)",
        exported: false,
        visibility: accessibility,
        async: isAsync(node),
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        params: countParams(node),
        depth: depth,
        body_text: node.text,
      };
    }

    // クラス宣言
    if (type === "class_declaration") {
      return {
        kind: "class",
        name: getChildText(node, "name", source) || "(anonymous)",
        exported: isExported(node),
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // インターフェース
    if (type === "interface_declaration") {
      const body = node.childForFieldName("body");
      return {
        kind: "interface",
        name: getChildText(node, "name", source) || "(anonymous)",
        exported: isExported(node),
        field_count: body ? countFields(body) : 0,
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // 型エイリアス
    if (type === "type_alias_declaration") {
      return {
        kind: "type_alias",
        name: getChildText(node, "name", source) || "(anonymous)",
        exported: isExported(node),
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // 変数宣言 (const Component = () => { ... } パターン含む)
    if (type === "lexical_declaration" && depth <= 2) {
      const declarators = node.namedChildren.filter(c => c.type === "variable_declarator");
      const entries = [];

      for (const decl of declarators) {
        const nameNode = decl.childForFieldName("name");
        const valueNode = decl.childForFieldName("value");
        const varName = nameNode ? nameNode.text : "(anonymous)";

        if (valueNode && (valueNode.type === "arrow_function" || valueNode.type === "function")) {
          // Arrow function / function expression
          const isComponent = /^[A-Z]/.test(varName);
          const isHook = /^use[A-Z]/.test(varName);
          entries.push({
            kind: isComponent ? "component" : isHook ? "hook" : "function",
            name: varName,
            exported: isExported(node),
            async: isAsync(valueNode),
            start_line: node.startPosition.row + 1,
            end_line: node.endPosition.row + 1,
            lines: node.endPosition.row - node.startPosition.row + 1,
            params: countParams(valueNode),
            depth: depth,
            body_text: valueNode.text,
          });
        } else if (valueNode && valueNode.type === "call_expression") {
          // const store = create(...) パターン (zustand等)
          const callName = valueNode.childForFieldName("function");
          const callee = callName ? callName.text : "";
          const isStore = callee === "create" || callee === "createStore";
          const isMemo = callee === "memo";
          const isCreateContext = callee === "createContext";

          if (isStore || isMemo || isCreateContext) {
            entries.push({
              kind: isStore ? "store" : isMemo ? "component" : "context",
              name: varName,
              exported: isExported(node),
              start_line: node.startPosition.row + 1,
              end_line: node.endPosition.row + 1,
              lines: node.endPosition.row - node.startPosition.row + 1,
              depth: depth,
              body_text: valueNode.text,
            });
          } else {
            entries.push({
              kind: "variable",
              name: varName,
              exported: isExported(node),
              start_line: node.startPosition.row + 1,
              end_line: node.endPosition.row + 1,
              lines: node.endPosition.row - node.startPosition.row + 1,
              depth: depth,
            });
          }
        } else {
          entries.push({
            kind: "variable",
            name: varName,
            exported: isExported(node),
            start_line: node.startPosition.row + 1,
            end_line: node.endPosition.row + 1,
            lines: node.endPosition.row - node.startPosition.row + 1,
            depth: depth,
          });
        }
      }
      return entries.length > 0 ? entries : null;
    }

    // Enum
    if (type === "enum_declaration") {
      return {
        kind: "enum",
        name: getChildText(node, "name", source) || "(anonymous)",
        exported: isExported(node),
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    return null;
  }

  function extractImport(node, source) {
    const sourceNode = node.children.find(c => c.type === "string");
    if (!sourceNode) return null;

    const modulePath = sourceNode.text.replace(/['"]/g, "");
    const names = [];

    const clause = node.children.find(c => c.type === "import_clause");
    if (clause) {
      // default import
      const defaultImport = clause.children.find(c => c.type === "identifier");
      if (defaultImport) names.push(defaultImport.text);

      // named imports
      const named = findDeep(clause, "import_specifier");
      for (const spec of named) {
        const nameNode = spec.childForFieldName("name");
        if (nameNode) names.push(nameNode.text);
      }

      // namespace import: import * as X from ...
      const ns = findDeep(clause, "namespace_import");
      for (const n of ns) {
        const id = n.children.find(c => c.type === "identifier");
        if (id) names.push(id.text);
      }
    }

    return {
      from: modulePath,
      names: names,
    };
  }

  function findDeep(node, type) {
    const results = [];
    function walk(n) {
      if (n.type === type) results.push(n);
      for (let i = 0; i < n.childCount; i++) walk(n.child(i));
    }
    walk(node);
    return results;
  }

  function getChildText(node, fieldName, source) {
    const child = node.childForFieldName(fieldName);
    return child ? child.text : null;
  }

  function isExported(node) {
    const parent = node.parent;
    return parent && parent.type === "export_statement";
  }

  function isAsync(node) {
    for (let i = 0; i < node.childCount; i++) {
      if (node.child(i).type === "async") return true;
    }
    return false;
  }

  function getAccessibility(node) {
    for (let i = 0; i < node.childCount; i++) {
      const text = node.child(i).text;
      if (text === "public" || text === "private" || text === "protected") {
        return text;
      }
    }
    return "public";
  }

  function countParams(node) {
    const params = node.childForFieldName("parameters");
    if (!params) return 0;
    return params.namedChildren.filter(c =>
      c.type === "required_parameter" ||
      c.type === "optional_parameter" ||
      c.type === "rest_parameter"
    ).length;
  }

  function countFields(body) {
    return body.namedChildren.filter(c =>
      c.type === "property_signature" ||
      c.type === "method_signature"
    ).length;
  }

  visit(tree.rootNode, 0);
  result.files.push(fileData);
}

// Phase 2: import → export のエッジを構築
const exportMap = {};

for (const file of result.files) {
  for (const node of file.nodes) {
    if (node.exported) {
      const key = `${file.name}:${node.name}`;
      exportMap[key] = file.path;
    }
  }
}

// Helper: resolve module path relative to root
function resolveModuleName(filePath, importFrom) {
  if (!importFrom.startsWith("./") && !importFrom.startsWith("../")) {
    return importFrom; // external or alias
  }
  const dir = path.dirname(filePath);
  const resolved = path.resolve(dir, importFrom);
  if (rootDir) {
    const absRoot = path.resolve(rootDir);
    if (resolved.startsWith(absRoot)) {
      let rel = resolved.slice(absRoot.length);
      if (rel.startsWith("/")) rel = rel.slice(1);
      return rel;
    }
  }
  const srcIdx = resolved.indexOf("/src/");
  if (srcIdx >= 0) {
    return resolved.substring(srcIdx + 5);
  }
  return path.basename(importFrom);
}

for (const file of result.files) {
  for (const imp of file.imports) {
    const moduleName = resolveModuleName(file.path, imp.from);

    for (const name of imp.names) {
      result.edges.push({
        from_file: file.name,
        to_file: moduleName,
        symbol: name,
        type: "import",
      });
    }
  }
}

// Phase 3: 関数呼び出し解析
// Build set of known internal file names for filtering
const internalNames = new Set(result.files.map(f => f.name));

for (const file of result.files) {
  const importedSymbols = {};
  for (const imp of file.imports) {
    const moduleName = resolveModuleName(file.path, imp.from);
    for (const name of imp.names) {
      importedSymbols[name] = moduleName;
    }
  }

  for (const node of file.nodes) {
    if (node.body_text && (node.kind === "function" || node.kind === "method" || node.kind === "component" || node.kind === "hook" || node.kind === "store")) {
      node.calls = [];
      for (const [symbol, fromModule] of Object.entries(importedSymbols)) {
        // 外部ライブラリはスキップ: プロジェクト内部の依存のみ検出
        // 判定: 解決後の名前が既知の内部ファイルに一致するか
        if (!internalNames.has(fromModule) && !fromModule.startsWith("./") && !fromModule.startsWith("../")) {
          continue;
        }

        const escaped = escapeRegex(symbol);
        let count = 0;

        // パターン1: 関数呼び出し symbol(
        const callMatches = node.body_text.match(new RegExp(`\\b${escaped}\\s*\\(`, "g"));
        if (callMatches) count += callMatches.length;

        // パターン2: JSXタグ <Symbol or <Symbol>
        const jsxMatches = node.body_text.match(new RegExp(`<${escaped}[\\s/>]`, "g"));
        if (jsxMatches) count += jsxMatches.length;

        // パターン3: プロパティアクセス symbol. (store/context使用)
        // symbol.getState() 等のパターン
        const dotMatches = node.body_text.match(new RegExp(`\\b${escaped}\\.`, "g"));
        if (dotMatches) count += dotMatches.length;

        if (count > 0) {
          node.calls.push({
            symbol: symbol,
            module: fromModule,
            count: count,
          });
        }
      }
      delete node.body_text;
    } else {
      delete node.body_text;
    }
  }
}

function escapeRegex(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

console.log(JSON.stringify(result, null, 2));
