import * as fs from "fs";
import { Crate, isMethod } from "./types";

const qt_widgets = JSON.parse(
    fs.readFileSync("./types/qt_widgets.json", { encoding: "utf8" }),
) as Crate;

let widgets = Object.entries(qt_widgets.index)
    .filter(([key, val]) => isMethod(val))
    .forEach(([key, val]) => console.log(val.name));
