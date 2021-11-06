import { ILayout, LayoutWidget } from "./layout";
import { Constructor, IWidget } from "../nodes";

interface ISingle {
    child: any;
}

export function Singleton<T extends Constructor<GtkWidget>>(
    tagName: string,
    $: T,
    opts: Partial<ILayout> = {},
): Constructor<IWidget> {
    return LayoutWidget(tagName, $, {
        onInsertWidgetAfter: ($, node) => {
            if ($.child) {
                throw new Error("Window accepts only one child");
            }
            $.child = node;
        },
        onAppendWidget: ($, node) => {
            if ($.child) {
                throw new Error("Window accepts only one child");
            }
            $.child = node;
        },
        onRemoveWidget: ($, node) => {
            $.child = null;
        },
        ...opts,
    });
}
