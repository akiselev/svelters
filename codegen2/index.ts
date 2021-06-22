import * as fs from "fs";
import { Crate, isImpl } from "./types";

const qt_widgets = JSON.parse(
    fs.readFileSync("./types/qt_widgets.json", { encoding: "utf8" }),
) as Crate;

let widgets = Object.entries(qt_widgets.index)
    .filter(([key, val]) => isImpl(val) && val?.inner?.trait)
    .map(
        ([key, val]) =>
            isImpl(val) && [val?.inner?.trait?.inner, val?.inner?.for?.inner],
    );

fs.writeFileSync("./from_impl.json", JSON.stringify(widgets, null, 2), {
    encoding: "utf8",
});
