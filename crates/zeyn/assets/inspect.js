/**
 * Source inspector of debug builds. Loaded only by pages whose nodes carry `data-zeyn-src` (the
 * Rust file, line and column that placed the node), which release builds never write.
 * Alt+Shift+I opens or closes it; Escape closes it. While open, the element under the pointer or
 * the focused element is outlined and a polite live region names its source line, followed by
 * the lines of the nodes that hold it. Its stylesheet loads on first open.
 * @module inspect
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const VERSION = new URL(import.meta.url).search;

/** Texts of the page, from `meta[name=zeyn-texts]`. */
function texts() {
  try {
    return JSON.parse(document.querySelector('meta[name="zeyn-texts"]')?.content ?? '{}');
  } catch {
    return {};
  }
}

let open = false;
let box = null;
let panel = null;
let current = null;

/** Creates the outline and the panel, and links the stylesheet once. */
function build() {
  if (!document.querySelector('link[data-zeyn-inspect]')) {
    const link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = `${BASE}/_zeyn/inspect.css${VERSION}`;
    link.dataset.zeynInspect = '';
    document.head.append(link);
  }
  const t = texts();
  box = document.createElement('div');
  box.className = 'z-inspect-box';
  box.setAttribute('aria-hidden', 'true');
  panel = document.createElement('div');
  panel.className = 'z-inspect';
  panel.setAttribute('role', 'status');
  panel.setAttribute('aria-live', 'polite');
  panel.setAttribute('aria-label', t['inspect.title'] ?? '');
  const list = document.createElement('ol');
  const hint = document.createElement('p');
  hint.className = 'z-inspect-hint';
  hint.textContent = t['inspect.hint'] ?? '';
  panel.append(list, hint);
}

/** The sources of `el` and of the nodes that hold it, nearest first, at most five. */
function chain(el) {
  const out = [];
  for (let n = el?.closest('[data-zeyn-src]'); n && out.length < 5; n = n.parentElement?.closest('[data-zeyn-src]')) {
    out.push(n);
  }
  return out;
}

/** Outlines the node of `el` and writes its sources into the panel. */
function show(el) {
  const nodes = chain(el);
  const target = nodes[0] ?? null;
  if (target === current) return place();
  current = target;
  const list = panel.querySelector('ol');
  list.replaceChildren(...nodes.map((n) => {
    const li = document.createElement('li');
    const code = document.createElement('code');
    code.textContent = n.dataset.zeynSrc;
    li.append(code);
    return li;
  }));
  if (!target) {
    const li = document.createElement('li');
    li.textContent = texts()['inspect.none'] ?? '';
    list.append(li);
  }
  place();
}

/** Moves the outline over the current node. */
function place() {
  if (!current || !current.isConnected) {
    box.hidden = true;
    return;
  }
  const r = current.getBoundingClientRect();
  box.hidden = false;
  box.style.top = `${r.top}px`;
  box.style.left = `${r.left}px`;
  box.style.width = `${r.width}px`;
  box.style.height = `${r.height}px`;
}

function onPointer(e) {
  show(e.target);
}

function onFocus(e) {
  show(e.target);
}

/** Opens or closes the inspector. */
function toggle(on) {
  open = on;
  if (on) {
    if (!box) build();
    document.body.append(box, panel);
    current = undefined;
    show(document.activeElement);
    document.addEventListener('pointermove', onPointer, { passive: true });
    document.addEventListener('focusin', onFocus);
    window.addEventListener('scroll', place, { passive: true, capture: true });
    window.addEventListener('resize', place, { passive: true });
  } else {
    box.remove();
    panel.remove();
    document.removeEventListener('pointermove', onPointer);
    document.removeEventListener('focusin', onFocus);
    window.removeEventListener('scroll', place, { capture: true });
    window.removeEventListener('resize', place);
  }
}

document.addEventListener('keydown', (e) => {
  if (e.altKey && e.shiftKey && !e.ctrlKey && !e.metaKey && e.code === 'KeyI') {
    e.preventDefault();
    toggle(!open);
  } else if (open && e.key === 'Escape') {
    toggle(false);
  }
});
