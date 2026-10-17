// Svelte's tests/html_equal.js (Svelte 5.57.2 @ f3de3f2, MIT), verbatim apart from types and the
// imports: `assert` is chai's (what vitest re-exports), the node type constants are inlined.
import { assert } from 'chai';

const ELEMENT_NODE = 1;
const TEXT_NODE = 3;
const COMMENT_NODE = 8;

function clean_children(node, opts) {
  let previous = null;
  let has_element_children = false;
  let template = node.nodeName.toUpperCase() === 'TEMPLATE' ? node : undefined;

  if (template) {
    const div = document.createElement('div');
    div.append(template.content);
    node = div;
  }

  const attributes = Array.from(node.attributes).sort((a, b) => (a.name < b.name ? -1 : 1));
  attributes.forEach((attr) => {
    if (attr.name !== 'xmlns') node.removeAttribute(attr.name);
  });
  attributes.forEach(({ name, value }) => {
    if (['onload', 'onerror', 'xmlns'].includes(name) && value === 'this.__e=event') return;
    if (name === 'class') value = value.replace(/svelte-\w+/, 'svelte-xyz123');
    node.setAttribute(name, value);
  });

  for (let child of [...node.childNodes]) {
    if (child.nodeType === TEXT_NODE) {
      let text = child;
      if (node.namespaceURI === 'http://www.w3.org/2000/svg' && node.tagName !== 'text' && node.tagName !== 'tspan') {
        node.removeChild(child);
        continue;
      }
      text.data = text.data.replace(/[^\S]+/g, ' ');
      if (previous && previous.nodeType === TEXT_NODE) {
        const prev = previous;
        prev.data += text.data;
        node.removeChild(text);
        text = prev;
        text.data = text.data.replace(/[^\S]+/g, ' ');
        continue;
      }
    }

    if (child.nodeType === COMMENT_NODE && !opts.preserveComments) {
      child.remove();
      continue;
    }

    if (child.nodeType === ELEMENT_NODE || child.nodeType === COMMENT_NODE) {
      if (previous?.nodeType === TEXT_NODE) {
        const prev = previous;
        prev.data = prev.data.replace(/^[^\S]+$/, '\n');
      } else if (previous?.nodeType === ELEMENT_NODE || previous?.nodeType === COMMENT_NODE) {
        node.insertBefore(document.createTextNode('\n'), child);
      }
      if (child.nodeType === ELEMENT_NODE) {
        has_element_children = true;
        clean_children(child, opts);
      }
    }

    previous = child;
  }

  if (node.firstChild && node.firstChild.nodeType === TEXT_NODE) {
    node.firstChild.data = node.firstChild.data.trimStart();
  }
  if (node.lastChild && node.lastChild.nodeType === TEXT_NODE) {
    node.lastChild.data = node.lastChild.data.trimEnd();
  }

  if (has_element_children && node.parentNode) {
    node.innerHTML = `\n\  ${node.innerHTML.replace(/\n/g, '\n  ')}\n`;
  }

  if (template) template.innerHTML = node.innerHTML;
}

export function normalize_html(window, html, { preserveComments = false } = {}) {
  try {
    const node = window.document.createElement('div');
    node.innerHTML = html.trim();
    clean_children(node, { preserveComments });
    return node.innerHTML;
  } catch (err) {
    throw new Error(`Failed to normalize HTML:\n${html}\nCause: ${err}`);
  }
}

export function normalize_new_line(html) {
  return html.replace(/\r\n/g, '\n');
}

export const assert_html_equal = (actual, expected, message) => {
  assert.deepEqual(normalize_html(window, actual), normalize_html(window, expected), message);
};

export const assert_html_equal_with_options = (actual, expected, { preserveComments, withoutNormalizeHtml }, message) => {
  assert.deepEqual(
    withoutNormalizeHtml
      ? normalize_new_line(actual.trim()).replace(/(<!(--)?.*?\2>)/g, preserveComments !== false ? '$1' : '')
      : normalize_html(window, actual.trim(), { preserveComments }),
    withoutNormalizeHtml
      ? normalize_new_line(expected.trim()).replace(/(<!(--)?.*?\2>)/g, preserveComments !== false ? '$1' : '')
      : normalize_html(window, expected.trim(), { preserveComments }),
    message,
  );
};
