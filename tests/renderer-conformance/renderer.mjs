// Custom renderer (sveltejs/svelte#18042 API) over a plain-JS "native widget" tree.
// Every mutating call is appended to `ops`, which is the exact stream a native backend
// (GTK4 / Qt / egui bridge) would receive. Read-only calls are tallied in `reads`.
import { createRenderer } from 'svelte/renderer';

let next_id = 1;
export const ops = [];
export const reads = {};
const read = (k) => (reads[k] = (reads[k] ?? 0) + 1);

const fmt = (v) =>
	typeof v === 'function' ? `fn ${v.name || '<anon>'}` :
	typeof v === 'string' ? JSON.stringify(v) :
	v && typeof v === 'object' ? `${Array.isArray(v) ? 'array' : 'object'} ${JSON.stringify(v)}` :
	String(v);
const ty = (v) => (v === null ? 'null' : Array.isArray(v) ? 'array' : typeof v);
export const label = (n) => (n == null ? 'null' : n.kind === 'element' ? `#${n.id}<${n.tag}>` : `#${n.id}${n.kind[0]}`);

export class WNode {
	constructor(kind, tag, value) {
		this.id = next_id++;
		this.kind = kind; // 'fragment' | 'element' | 'text' | 'comment'
		this.tag = tag;
		this.value = value ?? null;
		this.props = {}; // native property bag: values kept as-is (no stringification)
		this.children = [];
		this.parent = null;
		this.listeners = {};
	}
}

function detach(node) {
	const p = node.parent;
	if (!p) return;
	p.children.splice(p.children.indexOf(node), 1);
	node.parent = null;
}

export default createRenderer({
	createFragment() {
		const n = new WNode('fragment');
		ops.push(`create ${label(n)} fragment`);
		return n;
	},
	createElement(name) {
		const n = new WNode('element', name);
		ops.push(`create ${label(n)}`);
		return n;
	},
	createTextNode(data) {
		const n = new WNode('text', null, data);
		ops.push(`create ${label(n)} text ${JSON.stringify(data)}`);
		return n;
	},
	createComment(data) {
		const n = new WNode('comment', null, data);
		ops.push(`create ${label(n)} comment ${JSON.stringify(data)}`);
		return n;
	},
	nodeType(node) {
		read('nodeType');
		return node.kind;
	},
	getNodeValue(node) {
		read('getNodeValue');
		return node.value;
	},
	getAttribute(el, name) {
		read('getAttribute');
		return name in el.props ? el.props[name] : null;
	},
	setAttribute(el, key, value) {
		el.props[key] = value;
		ops.push(`set ${label(el)}.${key} = ${fmt(value)} [${ty(value)}]`);
	},
	removeAttribute(el, name) {
		delete el.props[name];
		ops.push(`unset ${label(el)}.${name}`);
	},
	hasAttribute(el, name) {
		read('hasAttribute');
		return name in el.props;
	},
	setText(node, text) {
		if (node.kind === 'element') {
			for (const c of node.children) c.parent = null;
			node.children = [];
			if (text) {
				const t = new WNode('text', null, text);
				t.parent = node;
				node.children.push(t);
			}
		} else node.value = text;
		ops.push(`text ${label(node)} = ${JSON.stringify(text)}`);
	},
	getFirstChild(el) {
		read('getFirstChild');
		return el.children[0] ?? null;
	},
	getLastChild(el) {
		read('getLastChild');
		return el.children[el.children.length - 1] ?? null;
	},
	getNextSibling(node) {
		read('getNextSibling');
		const p = node.parent;
		if (!p) return null;
		return p.children[p.children.indexOf(node) + 1] ?? null;
	},
	getParent(node) {
		read('getParent');
		return node.parent;
	},
	insert(parent, node, anchor) {
		const nodes = node.kind === 'fragment' ? [...node.children] : [node];
		if (node.kind === 'fragment') {
			for (const c of nodes) c.parent = null;
			node.children = [];
		}
		for (const n of nodes) {
			const moving = n.parent != null;
			detach(n);
			const i = anchor ? parent.children.indexOf(anchor) : parent.children.length;
			if (anchor && i < 0) throw new Error(`anchor ${label(anchor)} not a child of ${label(parent)}`);
			parent.children.splice(i, 0, n);
			n.parent = parent;
			ops.push(`${moving ? 'move' : 'insert'} ${label(n)} into ${label(parent)} before ${label(anchor)}`);
		}
	},
	remove(node) {
		ops.push(`remove ${label(node)}`);
		detach(node);
	},
	addEventListener(target, type, handler, options) {
		(target.listeners[type] ??= []).push(handler);
		ops.push(`listen ${label(target)} ${type}${options ? ' ' + JSON.stringify(options) : ''}`);
	},
	removeEventListener(target, type, handler) {
		const a = target.listeners[type];
		if (a) a.splice(a.indexOf(handler) >>> 0, 1);
		ops.push(`unlisten ${label(target)} ${type}`);
	}
});

// ---- host-side helpers (what the native bridge would do) ----

/** Simulate a native signal: call listeners with arbitrary args, return their results. */
export function emit(node, type, ...args) {
	return (node.listeners[type] ?? []).map((h) => h(...args));
}

export function root(tag = 'window') {
	return new WNode('element', tag);
}

export function find(node, pred) {
	if (typeof pred === 'string') {
		const tag = pred;
		pred = (n) => n.tag === tag;
	}
	if (pred(node)) return node;
	for (const c of node.children) {
		const r = find(c, pred);
		if (r) return r;
	}
	return null;
}

export function findAll(node, tag, out = []) {
	if (node.tag === tag) out.push(node);
	for (const c of node.children) findAll(c, tag, out);
	return out;
}

export function dump(node, d = 0) {
	const pad = '  '.repeat(d);
	if (node.kind === 'text') return `${pad}${JSON.stringify(node.value)}\n`;
	if (node.kind === 'comment') return `${pad}<!--${node.value}-->\n`;
	const props = Object.entries(node.props).map(([k, v]) => `${k}=${fmt(v)}`).join(' ');
	return `${pad}<${node.tag}${props ? ' ' + props : ''}>\n` + node.children.map((c) => dump(c, d + 1)).join('');
}

export function count(node, acc = { element: 0, text: 0, comment: 0 }) {
	for (const c of node.children) {
		acc[c.kind]++;
		count(c, acc);
	}
	return acc;
}

export { default as renderer } from './renderer.mjs';
