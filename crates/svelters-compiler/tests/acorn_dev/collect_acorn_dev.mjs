// Dev-only differential corpus for the acorn port (agent A). Not the official oracle (tools/oracle/).
//
// Usage:
//   node collect_acorn_dev.mjs <out.jsonl> [--acorn <dir of acorn package>] [--acorn-tests <acorn/test dir>]
//        [--js-root <dir>]...      full ASTs for every .js file (module, with and without preserveParens,
//                                  plus parseExpressionAt at expression starts)
//        [--hash-root <dir>]...    hashed ASTs for every .js/.mjs/.cjs file (large corpora)
// Each output line: {id, kind: "program"|"expr", code, options, pos?, expect} where expect is
// {ast, comments} | {hash} | {error, pos}, with BYTE offsets.
import fs from "node:fs"
import path from "node:path"
import vm from "node:vm"
import { createRequire } from "node:module"

const args = process.argv.slice(2)
const out = args[0]
let acornDir = "/home/user/svelters/node_modules/.pnpm/acorn@8.19.0/node_modules/acorn"
let acornTests = null
const jsRoots = []
const hashRoots = []
for (let i = 1; i < args.length; i++) {
  if (args[i] === "--acorn") acornDir = args[++i]
  else if (args[i] === "--acorn-tests") acornTests = args[++i]
  else if (args[i] === "--js-root") jsRoots.push(args[++i])
  else if (args[i] === "--hash-root") hashRoots.push(args[++i])
}
const require = createRequire(import.meta.url)
const acorn = require(acornDir)

const KEEP_OPTS = ["ecmaVersion", "sourceType", "allowReserved", "allowReturnOutsideFunction",
  "allowImportExportEverywhere", "allowAwaitOutsideFunction", "allowSuperOutsideMethod", "allowHashBang",
  "checkPrivateFields", "preserveParens", "strict"]

function cleanOptions(o) {
  const r = {}
  for (const k of KEEP_OPTS) if (o && Object.prototype.hasOwnProperty.call(o, k) && o[k] !== undefined) r[k] = o[k]
  return r
}

function byteMap(code) {
  // map[utf16 index] = utf8 byte offset
  const map = new Uint32Array(code.length + 1)
  let b = 0
  for (let i = 0; i < code.length; i++) {
    map[i] = b
    const c = code.charCodeAt(i)
    if (c < 0x80) b += 1
    else if (c < 0x800) b += 2
    else if (c >= 0xd800 && c <= 0xdbff && i + 1 < code.length) {
      const d = code.charCodeAt(i + 1)
      if (d >= 0xdc00 && d <= 0xdfff) { map[i + 1] = b; b += 4; i++; continue }
      b += 3
    } else b += 3
  }
  map[code.length] = b
  return map
}

function convert(node, map) {
  if (Array.isArray(node)) return node.map((n) => convert(n, map))
  if (node && typeof node === "object") {
    if (node instanceof RegExp) return {}
    const r = {}
    for (const k of Object.keys(node)) {
      if (k === "loc" || k === "range") continue
      let v = node[k]
      if ((k === "start" || k === "end") && typeof v === "number") v = map[v]
      else v = convert(v, map)
      r[k] = v
    }
    return r
  }
  if (typeof node === "bigint") return null
  if (typeof node === "string") return node.toWellFormed()
  if (typeof node === "number" && !Number.isFinite(node)) return null
  return node
}

// Canonical JSON (sorted keys, normalized numbers) + FNV-1a over UTF-16 units; mirrored in acorn_dev.rs.
function canon(v) {
  if (v === null) return "null"
  if (Array.isArray(v)) return "[" + v.map(canon).join(",") + "]"
  if (typeof v === "object") {
    const keys = Object.keys(v).sort()
    return "{" + keys.map((k) => JSON.stringify(k) + ":" + canon(v[k])).join(",") + "}"
  }
  if (typeof v === "number") {
    if (Number.isInteger(v) && Math.abs(v) < 2 ** 53) return String(v)
    return v.toExponential(16).replace("e+", "e")
  }
  return JSON.stringify(v)
}
function fnv(s) {
  let h = 0x811c9dc5
  for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0 }
  return h >>> 0
}

function run(code, options, pos, hash) {
  const comments = []
  const opts = { ...options, onComment: comments }
  const map = byteMap(code)
  try {
    const ast = pos === undefined ? acorn.parse(code, opts) : acorn.parseExpressionAt(code, pos, opts)
    const r = { ast: convert(ast, map), comments: convert(comments, map) }
    return hash ? { hash: fnv(canon(r)) } : r
  } catch (e) {
    if (!(e instanceof SyntaxError) && e.pos === undefined) return { error: String(e.message), pos: 0 }
    return { error: e.message.toWellFormed(), pos: e.pos === undefined ? 0 : map[e.pos] }
  }
}

const fd = fs.openSync(out, "w")
let n = 0
function add(id, code, options, extra = {}) {
  if (code.toWellFormed() !== code) return // lone surrogates cannot be represented in Rust strings
  const map = extra.pos === undefined ? null : byteMap(code)
  const expect = run(code, options, extra.pos, extra.hash)
  const line = { id, kind: extra.pos === undefined ? "program" : "expr", code, options, expect }
  if (map) line.pos = map[extra.pos]
  fs.writeSync(fd, JSON.stringify(line) + "\n")
  n++
}

if (acornTests) {
  const files = fs.readdirSync(acornTests).filter((f) => /^tests.*\.js$/.test(f)).sort()
  for (const f of files) {
    const collected = []
    const driver = {
      test: (code, ast, options) => collected.push({ code, options }),
      testFail: (code, message, options) => collected.push({ code, options }),
      testAssert: (code, assert, options) => collected.push({ code, options }),
    }
    const sandboxRequire = (p) => {
      if (p === "./driver.js") return driver
      if (p === "../acorn") return acorn
      if (p === "fs") return { writeFileSync() {} }
      return require(p)
    }
    const src = fs.readFileSync(path.join(acornTests, f), "utf8")
    const ctx = vm.createContext({ require: sandboxRequire, exports: {}, console })
    vm.runInContext(src, ctx, { filename: f })
    collected.forEach((t, i) => {
      const o = cleanOptions(t.options || {})
      if (!o.ecmaVersion) o.ecmaVersion = 5
      add(`${f}#${i}`, t.code, o)
    })
  }
}

function walk(dir, acc, re) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) walk(p, acc, re)
    else if (e.isFile() && re.test(e.name)) acc.push(p)
  }
  return acc
}

// Up to `max` expression start offsets (UTF-16) of an acorn AST.
function expressionStarts(ast, max) {
  const starts = new Set()
  const visit = (node) => {
    if (starts.size >= max || !node || typeof node !== "object") return
    if (Array.isArray(node)) return node.forEach(visit)
    if (typeof node.type === "string" && /Expression$|^Identifier$|^Literal$|^TemplateLiteral$/.test(node.type))
      starts.add(node.start)
    for (const k of Object.keys(node)) if (k !== "loc") visit(node[k])
  }
  visit(ast)
  return [...starts]
}

for (const root of jsRoots) {
  for (const f of walk(root, [], /\.js$/).sort()) {
    const code = fs.readFileSync(f, "utf8")
    const opts = { ecmaVersion: 16, sourceType: "module" }
    add(f, code, opts)
    add(f + "#parens", code, { ...opts, preserveParens: true })
    let ast
    try { ast = acorn.parse(code, opts) } catch { continue }
    for (const s of expressionStarts(ast, 40)) add(`${f}@${s}`, code, { ...opts, preserveParens: true }, { pos: s })
  }
}

for (const root of hashRoots) {
  for (const f of walk(root, [], /\.(js|mjs|cjs)$/).sort()) {
    if (/\/test(s)?\//.test(f)) continue
    const code = fs.readFileSync(f, "utf8")
    const sourceType = f.endsWith(".cjs") ? "script" : "module"
    add(f + "#hash", code, { ecmaVersion: 16, sourceType }, { hash: true })
  }
}

fs.closeSync(fd)
console.error(`wrote ${n} cases to ${out}`)
