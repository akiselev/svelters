#!/usr/bin/env node
// Generates corpus.json: regex (pattern, flags) pairs with real acorn's verdict per ecmaVersion.
// Usage: node generate.cjs <acorn 8.19.0 package dir> <acorn repo test/ dir>
// See README.md.
"use strict"
const fs = require("fs")
const path = require("path")
const vm = require("vm")

const [acornDir, testDir] = process.argv.slice(2)
if (!acornDir || !testDir) {
  console.error("usage: node generate.cjs <acorn package dir> <acorn repo test dir>")
  process.exit(2)
}
const acorn = require(path.resolve(acornDir))
if (acorn.version !== "8.19.0") throw new Error("expected acorn 8.19.0, got " + acorn.version)

const VERSIONS = [5, 6, 9, 10, 11, 12, 13, 14, 15, 16]

// Hook acorn's regexp validation: record every (pattern, flags) it sees and whether the pattern
// validation finished.
const pp = acorn.Parser.prototype
const origFlags = pp.validateRegExpFlags
const origPattern = pp.validateRegExpPattern
let seen = null
let validated = null
pp.validateRegExpFlags = function (state) {
  if (seen) seen.push([state.source, state.flags])
  validated = null
  return origFlags.call(this, state)
}
pp.validateRegExpPattern = function (state) {
  origPattern.call(this, state)
  validated = [state.source, state.flags]
}

const pairs = new Map()
function add(p, f, origin) {
  // Only well-formed UTF-16 (Rust &str) and patterns that can form a single regex literal.
  if (/\p{Cs}/u.test(p) || /\p{Cs}/u.test(f) || /[\n\r\u2028\u2029]/.test(p)) return
  const key = JSON.stringify([p, f])
  if (!pairs.has(key)) pairs.set(key, { p, f, origin })
}

// 1. Inputs of acorn's own test files (test()/testFail() stubbed), parsed with the hook on.
const inputs = []
for (const file of fs.readdirSync(testDir).filter((f) => /^tests.*\.js$/.test(f)).sort()) {
  const ctx = {
    test: (code, _ast, options) => inputs.push({ code, options, file }),
    testFail: (code, _msg, options) => inputs.push({ code, options, file }),
    testAssert: (code, _assert, options) => inputs.push({ code, options, file }),
    acorn,
    require: () => ({}),
    console,
  }
  try {
    vm.runInNewContext(fs.readFileSync(path.join(testDir, file), "utf8"), ctx, { filename: file })
  } catch (e) {
    console.error(`skipping rest of ${file}: ${e.message}`)
  }
}
for (const { code, options, file } of inputs) {
  if (typeof code !== "string") continue
  seen = []
  for (const ecmaVersion of [(options && options.ecmaVersion) || 5, "latest"]) {
    try {
      acorn.parse(code, Object.assign({}, options, { ecmaVersion, locations: false, onToken: undefined, onComment: undefined }))
    } catch (_e) {}
  }
  for (const [p, f] of seen) add(p, f, file)
  seen = null
}
const fromTests = pairs.size

// 2. Generated tricky patterns.
const handWritten = [
  "", "a", "(", ")", "[", "]", "{", "}", "|", "*", "+", "?", "a{", "a{1", "a{1,", "a{1}", "a{1,2}", "a{2,1}", "{1}", "a{,1}",
  "x{99999999999999999999,1}", "x{1,99999999999999999999}", "x{100000000000000000000,99999999999999999999}",
  "\\", "\\1", "(a)\\1", "(a)\\2", "\\99999999999999999999", "(a)\\01", "\\0", "\\00", "\\08", "\\377", "\\400", "\\8", "\\9",
  "\\c", "\\ca", "\\cA", "\\c0", "\\c_", "[\\c0]", "[\\c_]", "[\\c]", "[\\c*]", "\\k", "\\k<a>", "(?<a>)\\k<a>", "(?<a>)\\k<b>", "\\k<a>(?<a>)",
  "(?<a>)(?<a>)", "(?<a>)|(?<a>)", "((?<a>)|(?<a>))", "(?:(?<a>)|(?<b>))(?<a>)", "(?<a>a)|(?<a>b)\\k<a>",
  "(?<$>)", "(?<_>)", "(?<a1>)", "(?<1a>)", "(?<\\u0061>)", "(?<\\u{61}>)", "(?<\\u{1d49c}>)", "(?<\\ud835\\udc9c>)", "(?<𝒜>)", "(?<a𝒜>)",
  "(?<\\ud835>)", "(?<\\u{110000}>)", "(?<a\\u200c>)", "(?<a\\u200d>)", "(?<\\u200c>)", "(?<a", "(?<>)", "(?<a>", "(?", "(?i)", "(?=a)*", "(?!a)+", "(?<=a)*", "(?<!a)?",
  "(?=a){1}", "^*", "$+", "\\b*", "\\B{1}", "(?<ª>)", "(?<a·>)", "(?<à>)", "(?<̀>)", "(?<ℝ>)", "(?<a٠>)", "(?<〇>)", "(?<a\\u{e0100}>)", "(?<\u{20000}>)", "(?<a\u{1d7ce}>)",
  "(?i:a)", "(?m:a)", "(?s:a)", "(?ims:a)", "(?ii:a)", "(?-i:a)", "(?i-i:a)", "(?i-m:a)", "(?-:a)", "(?-)", "(?i-:a)", "(?-ii:a)", "(?x:a)", "(?i", "(?i-",
  "\\u", "\\u1", "\\u12", "\\u123", "\\u1234", "\\u{", "\\u{1}", "\\u{10FFFF}", "\\u{110000}", "\\u{FFFFFFFFFFFFFFFFFFFFFFFF}", "\\u{}", "\\ud83d\\ude00", "\\ud83d", "\\ude00\\ud83d",
  "\\x", "\\x1", "\\x41", "\\xg", "[\\x]", "\\p", "\\p{", "\\p{L}", "\\P{L}", "\\p{Letter}", "\\p{l}", "\\p{General_Category=L}", "\\p{gc=Lu}", "\\p{Script=Greek}", "\\p{sc=Grek}",
  "\\p{scx=Grek}", "\\p{Script_Extensions=Latin}", "\\p{Script=Kawi}", "\\p{Script=Toto}", "\\p{Script=Vith}", "\\p{Script=Yezi}", "\\p{Script=Wcho}", "\\p{Script=Dogr}", "\\p{Script=Unknown}",
  "\\p{Extended_Pictographic}", "\\p{EBase}", "\\p{ExtPict}", "\\p{RGI_Emoji}", "\\P{RGI_Emoji}", "[\\p{RGI_Emoji}]", "[^\\p{RGI_Emoji}]", "\\p{Basic_Emoji}", "\\p{Foo}", "\\p{Script=}", "\\p{=Greek}",
  "\\p{Script}", "\\p{ASCII=Y}", "\\p{__proto__}", "\\p{__proto__=L}", "\\p{toString}", "\\p{constructor=L}", "\\p{hasOwnProperty=L}", "\\p{Script=toString}", "\\p{L", "\\pL",
  "[a-z]", "[z-a]", "[\\d-z]", "[a-\\d]", "[\\w-\\d]", "[-a]", "[a-]", "[\\-]", "[\\b]", "[\\B]", "[\\1]", "[\\0]", "[\\01]", "[\\8]", "[^]", "[]", "[[]", "[]]", "[\\]", "[a",
  "[\\ud83d\\ude00-\\ud83d\\ude01]", "[😀-😁]", "[😁-😀]", "[\\u{1F600}-\\u{1F601}]", "[\\q{a}]", "[\\q{abc}]", "[^\\q{abc}]", "[^\\q{a}]", "[\\q{a|bc}]", "[\\q{}]", "[\\q{a|}]", "[\\q]", "[\\q{a]",
  "[a&&b]", "[a&&&b]", "[a&&]", "[&&a]", "[a--b]", "[a--]", "[a---b]", "[a&&b--c]", "[a--b&&c]", "[[a]&&[b]]", "[[a][b]]", "[a[b]]", "[^[^a]]", "[[^\\q{ab}]]", "[\\p{RGI_Emoji}--\\q{a}]",
  "[\\p{RGI_Emoji}&&\\q{ab}]", "[^\\p{RGI_Emoji}&&a]", "[!!]", "[!]", "[##]", "[a!!b]", "[\\!]", "[\\&]", "[\\#]", "[(]", "[)]", "[{]", "[}]", "[|]", "[\\/]", "[a-z--b]", "[a-z&&b]",
  "[\\d--\\w]", "[a-b-c]", "[\\s]", "[\\cA]", "[\\c1]", "[ab&&cd]", "[\\q{ab}&&a]", "[a&&\\q{ab}]", "[\\q{ab}--a]", "[[\\q{ab}]--a]", "[^[\\q{ab}]--a]", "[^[\\q{ab}]&&\\q{a}]",
  "a**", "a*?", "a+?", "a??", "a{1}?", "a{1}{1}", "(?:)", "(?:", "(?:a", "()", "(a", "a)", "a]", "a}", "a|*", ".", "\\/", "\\-", "\\a", "\\e", "\\_", "\\$", "\\😀",
  "😀", "😀*", "\\ud83d*", "[\\ud83d]", "\\u{1F600}+", "^$", "\\d\\D\\s\\S\\w\\W", "\\t\\n\\v\\f\\r", "\\cZ\\cz", "(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)\\10", "(a)\\10", "\\10(a)",
  "(?<a>.)\\k", "(?<a>.)\\k<", "(?<a>.)\\k<a", "\\k<a", "(?<a>)\\ka", "(?<a>)[\\k]", "[\\k<a>]", "(?<=(?<a>x))\\k<a>",
]
for (const p of handWritten) for (const f of ["", "u", "v", "i", "dgimsuy", "dgimsvy"]) add(p, f, "handwritten")
for (const f of ["g", "gg", "ii", "uv", "vu", "x", "d", "s", "y", "u", "v", "dv", "gimsuyd", "gimsuydv", "U", "é", "😀", "uu", "vv", "iu", "sd"]) add("a", f, "flags")

// Random combinations from a token alphabet (deterministic LCG).
const tokens = [
  "a", "b", "0", "1", "-", "^", "$", ".", "|", "*", "+", "?", "{", "}", "{1}", "{1,}", "{2,1}", ",", "(", ")", "(?:", "(?=", "(?!", "(?<=", "(?<!", "(?<a>", "(?<b>", "(?i:", "(?-s:", "(?im-s:",
  "[", "]", "[^", "\\", "\\1", "\\2", "\\0", "\\k<a>", "\\k<b>", "\\k", "\\d", "\\w", "\\b", "\\B", "\\c", "\\cx", "\\u0041", "\\u{41}", "\\ud83d", "\\ude00", "\\x4", "\\x41", "\\p{L}", "\\P{L}",
  "\\p{RGI_Emoji}", "\\q{", "\\q{ab}", "&&", "--", "&", "!!", "!", "😀", "é", "\\/", "\\-", "\\]", "\\[", "\\q", "\\8", "\\07", "<", ">", "=",
]
let seed = 0x5eed1234
function rnd(n) {
  seed = (Math.imul(seed, 1103515245) + 12345) >>> 0
  return (seed >>> 8) % n
}
for (let i = 0; i < 9000; i++) {
  const len = 1 + rnd(7)
  let p = ""
  for (let j = 0; j < len; j++) p += tokens[rnd(tokens.length)]
  add(p, ["", "u", "v", "i"][rnd(4)], "random")
}

// Verdicts.
function verdict(p, f, ecmaVersion) {
  validated = null
  seen = []
  let err = null
  try {
    acorn.parse("/" + p + "/" + f, { ecmaVersion })
  } catch (e) {
    err = e
  }
  const s = seen
  seen = null
  // The literal must be exactly our pattern/flags.
  if (s.length !== 1 || s[0][0] !== p || s[0][1] !== f) return undefined
  if (validated) return 0
  if (!err || err.pos !== 1 || !(err instanceof SyntaxError)) return undefined
  const msg = err.message.replace(/ \(1:1\)$/, "")
  const prefix = `Invalid regular expression: /${p}/: `
  if (msg.startsWith(prefix)) return ":" + msg.slice(prefix.length)
  if (msg === "Invalid regular expression flag" || msg === "Duplicate regular expression flag") return msg
  return undefined
}

const cases = []
let skipped = 0
for (const { p, f, origin } of pairs.values()) {
  const rs = VERSIONS.map((v) => verdict(p, f, v))
  if (rs.some((r) => r === undefined)) {
    skipped++
    continue
  }
  // Compact: one value when every version agrees.
  cases.push([p, f, rs.every((r) => r === rs[0]) ? rs[0] : rs, origin])
}
const out = {
  acorn: acorn.version,
  note: "Generated by generate.cjs; do not edit. Case = [pattern, flags, result or array of results per version, origin]; a single result applies to every version. Result: 0 = accepted, ':msg' = 'Invalid regular expression: /<pattern>/: msg', otherwise the flag error message.",
  versions: VERSIONS,
  cases,
}
const text = "{\n" + Object.entries(out).map(([k, v]) =>
  k === "cases" ? `"cases": [\n${v.map((c) => JSON.stringify(c)).join(",\n")}\n]` : `${JSON.stringify(k)}: ${JSON.stringify(v)}`
).join(",\n") + "\n}\n"
fs.writeFileSync(path.join(__dirname, "corpus.json"), text)
const accepted = cases.reduce((n, c) => n + (Array.isArray(c[2]) ? c[2].filter((r) => r === 0).length : c[2] === 0 ? VERSIONS.length : 0), 0)
console.log(`${cases.length} cases (${fromTests} pairs from acorn tests), ${cases.length * VERSIONS.length} verdicts, ${accepted} accepted; skipped ${skipped}; ${text.length} bytes`)
