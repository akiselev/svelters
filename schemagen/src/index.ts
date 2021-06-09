import * as fs from "fs-jetpack"
import { Parser, Header, formatMatches, crawl } from "./helpers"
import { CppParser, SyntaxNode } from "./Parser"
import * as q from "./helpers/query";

const SCAN_PATH = "/Users/kiselev/Qt/5.15.2/clang_64"
const parser = new Parser()
SCAN_PATH //?
const headers = fs
    .find(SCAN_PATH, { matching: "**/*.@(h|hpp)" })
    .filter((path) => path.indexOf("/private/") === -1)
const path = headers.find((path) => path.indexOf("qpushbutton") > -1)
const btn = parser.parse(path!)

console.log(path)
let t = btn.tree.rootNode as SyntaxNode


// fs.write("output.json", JSON.stringify(crawl(t), null, 4))

let res = btn.query(q.findQtWidgets)//?
fs.write("result.json", JSON.stringify(crawl(res[0].captures[0].node), null, 4))
res = btn.query_capture(q.findQtWidgets)
res //?

