import { initBaseNode, initSingleton, initWidget } from "./base";

export class ListViewProxy {
    private _pending: ((self: any) => void)[];
    private _child: any;

    constructor(private extra = {}) {
        this._child = null;
        this._pending = [];
    }

    get svExtra() {
        return this.extra;
    }

    get child() {
        return this._child;
    }

    set child(child: any) {
        this._child = child;
        if (this._pending.length > 0) {
            this._pending.forEach((func) => func(child));
            this._pending = [];
        }
    }

    private callChild =
        (funcName: string) =>
        (...args: any[]) => {
            if (this._child) {
                this._child[funcName](...args);
            } else {
                this._pending.push((child) => child[funcName](...args));
            }
        };

    getAttribute = (name: string) => {
        return this[name];
    };

    setAttribute = (name: string, value: any) => {
        this[name] = value;
    };

    insertBefore = this.callChild("insertBefore");
    appendChild = this.callChild("appendChild");
    removeChild = this.callChild("removeChild");
}

initBaseNode("ListViewProxy", ListViewProxy, false);
