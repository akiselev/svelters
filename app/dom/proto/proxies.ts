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

    get target() {
        return this._child;
    }

    set target(target: any) {
        this._child = target;
        if (this._pending.length > 0) {
            this._pending.forEach((func) => func(target));
            this._pending = [];
        }
    }

    private callChild =
        (funcName: string) =>
        (...args: any[]) => {
            if (this._child) {
                this._child[funcName](...args);
            } else {
                this._pending.push((target) => target[funcName](...args));
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
