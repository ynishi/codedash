/**
 * parse_rust.js - Rustファイルをtree-sitterで解析 → JSON出力
 *
 * codedash用ASTデータ(ast_data.json)を生成する。
 * parse_tsx.jsと同じスキーマを出力し、Luaエンジン側の変更なしで動作する。
 *
 * 抽出対象:
 * - function_item (トップレベル関数 / impl内メソッド)
 * - struct_item, enum_item, trait_item, impl_item
 * - type_item, const_item, static_item, mod_item, macro_definition
 * - use_declaration (内部import → edgeとして出力)
 *
 * Usage: node tools/parse_rust.js [--root=<dir>] file1.rs [file2.rs ...] > ast_data.json
 */

const Parser = require("tree-sitter");
const RustLang = require("tree-sitter-rust");
const fs = require("fs");
const path = require("path");

const parser = new Parser();
parser.setLanguage(RustLang);

// --root オプション解析
let rootDir = null;
const files = [];
for (const arg of process.argv.slice(2)) {
  if (arg.startsWith("--root=")) {
    rootDir = arg.slice(7);
    if (!rootDir.endsWith("/")) rootDir += "/";
  } else {
    files.push(arg);
  }
}
if (files.length === 0) {
  console.error("Usage: node parse_rust.js [--root=<dir>] <file1.rs> [file2.rs ...]");
  process.exit(1);
}

const result = {
  files: [],
  edges: [],
};

for (const filePath of files) {
  const source = fs.readFileSync(filePath, "utf-8");

  let tree;
  try {
    tree = parser.parse(source);
  } catch (e) {
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

  // 相対パス構築
  let relPath;
  if (rootDir) {
    const absFile = path.resolve(filePath);
    const absRoot = path.resolve(rootDir);
    relPath = absFile.startsWith(absRoot) ? absFile.slice(absRoot.length) : absFile;
    relPath = relPath.replace(/\.rs$/, "");
    if (relPath.startsWith("/")) relPath = relPath.slice(1);
  } else {
    // src/, examples/, benches/, tests/ 等のRust標準ディレクトリを検出してstrip
    relPath = filePath
      .replace(/.*\/(src|examples|benches|tests)\//, "$1/")
      .replace(/\.rs$/, "");
  }

  const fileData = {
    path: filePath,
    name: relPath,
    nodes: [],
    imports: [],
  };

  // ============================================================
  // AST走査 (Phase 1)
  // ============================================================

  function visit(node, depth, implTarget) {
    const entries = extractNode(node, source, depth, implTarget);
    if (entries) {
      for (const entry of Array.isArray(entries) ? entries : [entries]) {
        fileData.nodes.push(entry);
      }
    }

    // impl_item内の関数は method として扱う
    let childImplTarget = implTarget;
    if (node.type === "impl_item") {
      childImplTarget = getImplTarget(node);
    }

    for (let i = 0; i < node.childCount; i++) {
      visit(node.child(i), depth + 1, childImplTarget);
    }
  }

  function extractNode(node, source, depth, implTarget) {
    const type = node.type;

    // use宣言 → imports
    if (type === "use_declaration") {
      const importInfo = extractUse(node);
      if (importInfo) fileData.imports.push(importInfo);
      return null;
    }

    // 関数 / メソッド
    if (type === "function_item") {
      const name = getChildText(node, "name") || "(anonymous)";
      const vis = getVisibility(node);
      const isMethod = !!implTarget;
      return {
        kind: isMethod ? "method" : "function",
        name: name,
        exported: vis.exported,
        visibility: vis.text,
        is_async: hasKeyword(node, "async"),
        is_unsafe: hasKeyword(node, "unsafe"),
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        params: countParams(node),
        depth: depth,
        cyclomatic: computeCyclomatic(node),
        body_text: node.text,
      };
    }

    // struct
    if (type === "struct_item") {
      const vis = getVisibility(node);
      const body = node.childForFieldName("body");
      return {
        kind: "struct",
        name: getChildText(node, "name") || "(anonymous)",
        exported: vis.exported,
        visibility: vis.text,
        field_count: body ? countStructFields(body) : countTupleFields(node),
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // enum
    if (type === "enum_item") {
      const vis = getVisibility(node);
      const body = node.childForFieldName("body");
      return {
        kind: "enum",
        name: getChildText(node, "name") || "(anonymous)",
        exported: vis.exported,
        visibility: vis.text,
        field_count: body ? countEnumVariants(body) : 0,
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // trait
    if (type === "trait_item") {
      const vis = getVisibility(node);
      const body = node.childForFieldName("body");
      return {
        kind: "trait",
        name: getChildText(node, "name") || "(anonymous)",
        exported: vis.exported,
        visibility: vis.text,
        field_count: body ? countTraitItems(body) : 0,
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // impl ブロック (コンテナとして出力 → loader.luaが親子関係を構築)
    if (type === "impl_item") {
      const target = getImplTarget(node);
      const traitName = getImplTrait(node);
      return {
        kind: "impl",
        name: target || "(anonymous)",
        trait_name: traitName,
        exported: false,
        visibility: "private",
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // type alias
    if (type === "type_item") {
      const vis = getVisibility(node);
      return {
        kind: "type_alias",
        name: getChildText(node, "name") || "(anonymous)",
        exported: vis.exported,
        visibility: vis.text,
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // const
    if (type === "const_item") {
      const vis = getVisibility(node);
      return {
        kind: "const",
        name: getChildText(node, "name") || "(anonymous)",
        exported: vis.exported,
        visibility: vis.text,
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // static
    if (type === "static_item") {
      const vis = getVisibility(node);
      return {
        kind: "static",
        name: getChildText(node, "name") || "(anonymous)",
        exported: vis.exported,
        visibility: vis.text,
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // module
    if (type === "mod_item") {
      const vis = getVisibility(node);
      return {
        kind: "module",
        name: getChildText(node, "name") || "(anonymous)",
        exported: vis.exported,
        visibility: vis.text,
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    // macro_rules!
    if (type === "macro_definition") {
      const vis = getVisibility(node);
      return {
        kind: "macro",
        name: getChildText(node, "name") || "(anonymous)",
        exported: vis.exported,
        visibility: vis.text,
        start_line: node.startPosition.row + 1,
        end_line: node.endPosition.row + 1,
        lines: node.endPosition.row - node.startPosition.row + 1,
        depth: depth,
      };
    }

    return null;
  }

  // ============================================================
  // ヘルパー関数
  // ============================================================

  function getChildText(node, fieldName) {
    const child = node.childForFieldName(fieldName);
    return child ? child.text : null;
  }

  function getVisibility(node) {
    for (let i = 0; i < node.childCount; i++) {
      const child = node.child(i);
      if (child.type === "visibility_modifier") {
        const text = child.text;
        return {
          exported: text === "pub",
          text: text, // "pub", "pub(crate)", "pub(super)", etc.
        };
      }
    }
    return { exported: false, text: "private" };
  }

  function hasKeyword(node, keyword) {
    for (let i = 0; i < node.childCount; i++) {
      if (node.child(i).type === keyword) return true;
    }
    return false;
  }

  /**
   * パラメータ数カウント (selfは除外)
   */
  function countParams(node) {
    const params = node.childForFieldName("parameters");
    if (!params) return 0;
    let count = 0;
    for (let i = 0; i < params.namedChildCount; i++) {
      const child = params.namedChildren[i];
      // self_parameter は除外
      if (child.type === "parameter") {
        count++;
      }
    }
    return count;
  }

  /**
   * Named struct のフィールド数
   */
  function countStructFields(body) {
    if (body.type !== "field_declaration_list") return 0;
    return body.namedChildren.filter(c => c.type === "field_declaration").length;
  }

  /**
   * Tuple struct のフィールド数 (e.g., struct Point(f64, f64))
   */
  function countTupleFields(structNode) {
    for (let i = 0; i < structNode.childCount; i++) {
      const child = structNode.child(i);
      if (child.type === "ordered_field_declaration_list") {
        return child.namedChildren.length;
      }
    }
    return 0;
  }

  /**
   * Enum のバリアント数
   */
  function countEnumVariants(body) {
    if (body.type !== "enum_variant_list") return 0;
    return body.namedChildren.filter(c => c.type === "enum_variant").length;
  }

  /**
   * Trait 内のメソッド・関連型数
   */
  function countTraitItems(body) {
    if (body.type !== "declaration_list") return 0;
    return body.namedChildren.filter(c =>
      c.type === "function_item" ||
      c.type === "function_signature_item" ||
      c.type === "associated_type" ||
      c.type === "const_item"
    ).length;
  }

  /**
   * impl の対象型名を取得
   * impl Foo { ... } → "Foo"
   * impl Trait for Foo { ... } → "Foo"
   */
  function getImplTarget(node) {
    const typeNode = node.childForFieldName("type");
    if (!typeNode) return null;
    // ジェネリクスを除いた型名のみ取得
    if (typeNode.type === "type_identifier") {
      return typeNode.text;
    }
    if (typeNode.type === "generic_type") {
      const base = typeNode.childForFieldName("type");
      return base ? base.text : typeNode.text;
    }
    // スライス、参照等の場合はテキスト全体
    return typeNode.text;
  }

  /**
   * impl Trait for Foo の Trait名を取得
   */
  function getImplTrait(node) {
    const traitNode = node.childForFieldName("trait");
    if (!traitNode) return null;
    if (traitNode.type === "type_identifier") {
      return traitNode.text;
    }
    if (traitNode.type === "generic_type") {
      const base = traitNode.childForFieldName("type");
      return base ? base.text : traitNode.text;
    }
    return traitNode.text;
  }

  /**
   * McCabe cyclomatic complexity 計算
   *
   * 基準値: 1 (直線パス)
   * 加算: if, while, for, loop, match arm, &&, ||, ? 演算子
   * match_expression ごとに -1 (1 arm が基底パスなので)
   */
  function computeCyclomatic(fnNode) {
    let complexity = 1;
    const body = fnNode.childForFieldName("body");
    if (!body) return complexity;

    function walk(node) {
      switch (node.type) {
        case "if_expression":
          complexity++;
          break;
        case "while_expression":
        case "for_expression":
        case "loop_expression":
          complexity++;
          break;
        case "match_expression":
          // match自体で -1 し、各armで +1 → 結果: arms - 1
          complexity--;
          break;
        case "match_arm":
          complexity++;
          break;
        case "try_expression": // ? 演算子
          complexity++;
          break;
        case "binary_expression": {
          // && / || は短絡評価で分岐
          const op = node.child(1);
          if (op && (op.text === "&&" || op.text === "||")) {
            complexity++;
          }
          break;
        }
      }
      for (let i = 0; i < node.childCount; i++) {
        walk(node.child(i));
      }
    }

    walk(body);
    return complexity;
  }

  // ============================================================
  // use宣言の解析
  // ============================================================

  function extractUse(node) {
    const text = node.text.trim();

    // 内部インポートのみ (crate::, super::, self::)
    if (!text.match(/^use\s+(crate|super|self)::/)) {
      return null;
    }

    // 'use ' と ';' を除去
    let usePath = text.replace(/^use\s+/, "").replace(/;\s*$/, "").trim();

    // glob import はスキップ
    if (usePath.endsWith("::*")) {
      return null;
    }

    // グループ import: use crate::module::{Foo, Bar, sub::Baz};
    const groupMatch = usePath.match(/^(.+)::\{(.+)\}$/s);
    if (groupMatch) {
      const basePath = groupMatch[1];
      const names = groupMatch[2]
        .split(",")
        .map(s => {
          s = s.trim();
          // 'self' → モジュール自体のimport、スキップ
          if (s === "self") return null;
          // 'Name as Alias' → Alias を使う
          const asMatch = s.match(/^([\w:]+)\s+as\s+(\w+)$/);
          if (asMatch) return asMatch[2];
          // nested path: 'sub::Foo' → 末尾の識別子 'Foo' を取得
          const parts = s.split("::");
          const last = parts[parts.length - 1];
          // ネストされたグループ ({...}) はスキップ
          if (last.includes("{") || last.includes("}")) return null;
          return last;
        })
        .filter(s => s != null && s.length > 0);
      if (names.length > 0) {
        return { from: normalizeUsePath(basePath), names };
      }
      return null;
    }

    // エイリアス import: use crate::module::Type as Alias;
    const asMatch = usePath.match(/^(.+)::(\w+)\s+as\s+(\w+)$/);
    if (asMatch) {
      return { from: normalizeUsePath(asMatch[1]), names: [asMatch[3]] };
    }

    // 単純 import: use crate::module::Type;
    const simpleMatch = usePath.match(/^(.+)::(\w+)$/);
    if (simpleMatch) {
      return { from: normalizeUsePath(simpleMatch[1]), names: [simpleMatch[2]] };
    }

    return null;
  }

  /**
   * use パスを正規化
   * crate::foo::bar → foo/bar
   * super::foo → ../foo  (相対パスとして記録)
   * self::foo → ./foo
   */
  function normalizeUsePath(usePath) {
    if (usePath.startsWith("crate::")) {
      return usePath.slice(7).replace(/::/g, "/");
    }
    if (usePath.startsWith("super::")) {
      return "../" + usePath.slice(7).replace(/::/g, "/");
    }
    if (usePath.startsWith("self::")) {
      return "./" + usePath.slice(6).replace(/::/g, "/");
    }
    return usePath.replace(/::/g, "/");
  }

  visit(tree.rootNode, 0, null);
  result.files.push(fileData);
}

// ============================================================
// Phase 2: use → export のエッジを構築
// ============================================================

const exportMap = {};
for (const file of result.files) {
  for (const node of file.nodes) {
    if (node.exported) {
      const key = `${file.name}:${node.name}`;
      exportMap[key] = file.path;
    }
  }
}

// 既知の内部ファイル名セット
const internalNames = new Set(result.files.map(f => f.name));

for (const file of result.files) {
  for (const imp of file.imports) {
    for (const name of imp.names) {
      result.edges.push({
        from_file: file.name,
        to_file: imp.from,
        symbol: name,
        type: "import",
      });
    }
  }
}

// ============================================================
// Phase 3: 関数呼び出し解析
// ============================================================

// RegExpキャッシュ: シンボルごとに事前コンパイル
const regexCache = {};
function getSymbolRegexes(symbol) {
  if (regexCache[symbol]) return regexCache[symbol];
  const escaped = escapeRegex(symbol);
  regexCache[symbol] = {
    call:  new RegExp(`\\b${escaped}\\s*\\(`, "g"),
    type:  new RegExp(`[:<>]\\s*${escaped}\\b`, "g"),
    assoc: new RegExp(`\\b${escaped}::`, "g"),
    macro: new RegExp(`\\b${escaped}!\\s*[\\(\\[\\{]`, "g"),
  };
  return regexCache[symbol];
}

// calls解析対象のkind (macro含む)
const CALLABLE_KINDS = new Set(["function", "method", "macro"]);

for (const file of result.files) {
  // このファイルのimportシンボルマップ
  const importedSymbols = {};
  for (const imp of file.imports) {
    for (const name of imp.names) {
      importedSymbols[name] = imp.from;
    }
  }

  // 内部モジュールのシンボルのみ抽出 (ループ内で毎回判定しない)
  const internalSymbols = [];
  for (const [symbol, fromModule] of Object.entries(importedSymbols)) {
    if (internalNames.has(fromModule) ||
        fromModule.startsWith("./") ||
        fromModule.startsWith("../")) {
      internalSymbols.push({ symbol, fromModule, re: getSymbolRegexes(symbol) });
    }
  }

  for (const node of file.nodes) {
    if (node.body_text && CALLABLE_KINDS.has(node.kind)) {
      node.calls = [];
      for (const { symbol, fromModule, re } of internalSymbols) {
        let count = 0;

        // パターン1: 関数呼び出し symbol(
        re.call.lastIndex = 0;
        const callMatches = node.body_text.match(re.call);
        if (callMatches) count += callMatches.length;

        // パターン2: 型として使用 (型コンテキスト: <Symbol>, : Symbol, -> Symbol)
        re.type.lastIndex = 0;
        const typeMatches = node.body_text.match(re.type);
        if (typeMatches) count += typeMatches.length;

        // パターン3: 関連関数呼び出し Symbol::method(
        re.assoc.lastIndex = 0;
        const assocMatches = node.body_text.match(re.assoc);
        if (assocMatches) count += assocMatches.length;

        // パターン4: マクロ呼び出し symbol!(
        re.macro.lastIndex = 0;
        const macroMatches = node.body_text.match(re.macro);
        if (macroMatches) count += macroMatches.length;

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
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

console.log(JSON.stringify(result, null, 2));
