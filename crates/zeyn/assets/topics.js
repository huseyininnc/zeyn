/**
 * Topics: nodes that refresh in place (`.listens`), filter controls that drive them (`.drives`)
 * and the `refresh` effect (`Effect::refresh`). Loaded only by pages that hold
 * `[data-zeyn-listens]` or `[data-zeyn-drives]`.
 *
 * Topics named within one animation frame are handled once. A listener that holds a grid reloads
 * its grids (`zeyn:reload`); every other listener of a screen is rendered again with one
 * `POST /_zeyn/nodes`, and a newer request for the same screen cancels the one still running and
 * takes over its nodes. A driving control puts its value into the screen's params
 * (`data-zeyn-params`), replaces the address with the new params when the screen is the page's
 * main screen, and refreshes the listeners of its topics; grids then start on their first page.
 * Only topic names and params travel; the server checks access for every request. Focus and open
 * `<details>` survive a refresh when the new content has the same element (id, test id or row
 * key). `zeyn:topics` with `detail.topics` lets other scripts (`live.js`) name topics.
 * @module topics
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');

/** Topics named since the last frame, whether a control drove them, and nodes to render too. */
const pending = { topics: new Set(), reset: false, extra: new Set() };
let frame = 0;
/** @type {WeakMap<Element, {ctl: AbortController, nodes: Element[]}>} The refresh running per screen. */
const running = new WeakMap();

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/** Posts JSON with the CSRF header. Resolves to the parsed body, or `null` on any failure. */
async function post(path, body, signal) {
  try {
    const res = await fetch(BASE + path, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify(body),
      signal,
    });
    return res.ok ? await res.json() : null;
  } catch {
    return null;
  }
}

/** The params of a screen root, or `null`. */
function paramsOf(screen) {
  try {
    return JSON.parse(screen?.dataset.zeynParams ?? 'null');
  } catch {
    return null;
  }
}

/** Adds topics to the next frame's refresh. */
function queue(topics, reset, extra) {
  for (const t of Array.isArray(topics) ? topics : []) if (typeof t === 'string' && t) pending.topics.add(t);
  if (reset) pending.reset = true;
  if (extra) pending.extra.add(extra);
  if (pending.topics.size) frame ||= requestAnimationFrame(flush);
}

/** Refreshes the listeners of the topics named since the last frame. */
function flush() {
  frame = 0;
  const topics = pending.topics;
  const { reset, extra } = pending;
  pending.topics = new Set();
  pending.reset = false;
  pending.extra = new Set();
  const hit = [...document.querySelectorAll('[data-zeyn-listens]')].filter((el) => el.dataset.zeynListens.split(',').some((t) => topics.has(t)));
  const batches = new Map();
  const add = (node) => {
    const screen = node?.closest('[data-zeyn-screen]');
    if (!screen) return;
    if (!batches.has(screen)) batches.set(screen, []);
    batches.get(screen).push(node);
  };
  for (const el of hit) {
    if (hit.some((o) => o !== el && o.contains(el))) continue;
    if (el.querySelector('[data-zeyn-grid]')) el.dispatchEvent(new CustomEvent('zeyn:reload', { bubbles: true, detail: { reset } }));
    else add(el.closest('[data-zeyn-node]'));
  }
  for (const node of extra) if (node.isConnected && !hit.some((el) => el.contains(node))) add(node);
  for (const [screen, nodes] of batches) render(screen, nodes);
}

/** Renders nodes of one screen again; a newer call for the screen cancels this one and takes its nodes. */
async function render(screen, nodes) {
  const before = running.get(screen);
  if (before) {
    before.ctl.abort();
    for (const n of before.nodes) if (n.isConnected && !nodes.includes(n)) nodes.push(n);
  }
  const ctl = new AbortController();
  running.set(screen, { ctl, nodes });
  for (const n of nodes) {
    n.zeynBusy = ctl;
    n.setAttribute('aria-busy', 'true');
  }
  const body = await post('/_zeyn/nodes', { screen: screen.dataset.zeynScreen, params: paramsOf(screen), nodes: nodes.map((n) => n.dataset.zeynNode) }, ctl.signal);
  if (ctl.signal.aborted) return;
  running.delete(screen);
  for (const n of nodes) {
    if (n.zeynBusy !== ctl) continue;
    n.removeAttribute('aria-busy');
    const html = body?.nodes?.[n.dataset.zeynNode];
    if (typeof html === 'string' && n.isConnected) replace(n, html);
  }
}

/**
 * A way to find `el` again in new content: by id, test id, row key and position in the row, or
 * else its position in the node's element tree.
 */
function keyOf(el, root) {
  if (el.id) return { sel: `#${CSS.escape(el.id)}` };
  if (el.dataset.testid) return { sel: `[data-testid="${CSS.escape(el.dataset.testid)}"]` };
  const row = el.closest('[data-zeyn-key]');
  if (!row || !root.contains(row)) {
    const path = [];
    for (let at = el; at && at !== root; at = at.parentElement) path.unshift([...at.parentElement.children].indexOf(at));
    return { path };
  }
  const sel = `[data-zeyn-key="${CSS.escape(row.dataset.zeynKey)}"]`;
  return row === el ? { sel } : { sel, tag: el.tagName, i: [...row.querySelectorAll(el.tagName)].indexOf(el) };
}

/** The element a key names inside `root`. */
function find(root, key) {
  if (key?.path) return key.path.reduce((at, i) => at?.children[i] ?? null, root);
  const base = key && root.querySelector(key.sel);
  if (!base || key.tag == null) return base;
  return base.querySelectorAll(key.tag)[key.i] ?? base;
}

/** Swaps a node for its new HTML, keeping focus and open details, and sets up what it holds. */
function replace(node, html) {
  const t = document.createElement('template');
  t.innerHTML = html;
  const fresh = t.content.firstElementChild;
  if (!fresh) return;
  const active = document.activeElement;
  const focus = active && node.contains(active) ? keyOf(active, node) : null;
  const open = [...node.querySelectorAll('details[open]')].map((d) => keyOf(d, node)).filter(Boolean);
  node.replaceWith(fresh);
  for (const key of open) find(fresh, key)?.setAttribute('open', '');
  find(fresh, focus)?.focus({ preventScroll: true });
  document.dispatchEvent(new CustomEvent('zeyn:content', { detail: fresh }));
}

/** Values of a filter form's named fields, typed by `data-zeyn-kinds`; empty fields are `null`. */
function formValues(form) {
  let kinds = {};
  try {
    kinds = JSON.parse(form.dataset.zeynKinds ?? '{}') ?? {};
  } catch {}
  const out = {};
  for (const el of form.elements) {
    if (!el.name || el.disabled || el.type === 'submit' || el.type === 'button') continue;
    let v;
    if (el.type === 'checkbox') v = kinds[el.name] === 'bool' ? el.checked : el.checked ? el.value : null;
    else if (el.type === 'radio') {
      if (!el.checked) continue;
      v = el.value;
    } else if (el.matches('[data-zeyn-date-input]')) {
      if (el.value.trim() && !el.dataset.iso) continue;
      v = el.value.trim() ? el.dataset.iso : null;
    } else v = el.value;
    if (v === '') v = null;
    else if (kinds[el.name] === 'number' && typeof v === 'string') {
      const n = Number(v);
      if (Number.isFinite(n)) v = n;
    }
    out[el.name] = v;
  }
  return out;
}

/** Keeps the address of the main screen in step with driven params that are screen params. */
function syncUrl(screen, values, href) {
  if (screen.closest('dialog') || !screen.closest('.z-main')) return;
  if (href) {
    history.replaceState(history.state, '', new URL(href, location.href).href);
    return;
  }
  const url = new URL(location.href);
  let q = {};
  try {
    q = JSON.parse(url.searchParams.get('params') ?? '{}') ?? {};
  } catch {}
  for (const [k, v] of Object.entries(values)) {
    if (v == null || v === '') delete q[k];
    else q[k] = v;
  }
  if (Object.keys(q).length) url.searchParams.set('params', JSON.stringify(q));
  else url.searchParams.delete('params');
  history.replaceState(history.state, '', url.href);
}

/**
 * Puts a driving control's values into its screen's params and refreshes the listeners of its
 * topics. When the screen came with params, only keys they already have are taken (a screen
 * opened without params has `null`, and takes every value); nothing happens when nothing changed.
 * The server reads the result as the screen's typed params.
 */
function drive(control, values, href, again) {
  const screen = control.closest('[data-zeyn-screen]');
  if (!screen) return;
  const raw = paramsOf(screen);
  if (raw != null && (typeof raw !== 'object' || Array.isArray(raw))) return;
  const params = raw ?? {};
  const taken = raw == null ? values : Object.fromEntries(Object.entries(values).filter(([k]) => k in params));
  const next = { ...params, ...taken };
  if (JSON.stringify(next) === JSON.stringify(params)) return;
  screen.dataset.zeynParams = JSON.stringify(next);
  syncUrl(screen, taken, href);
  queue(control.dataset.zeynDrives.split(','), true, again ? control.closest('[data-zeyn-node]') : null);
}

document.addEventListener('zeyn:effect', (e) => {
  const effect = e.detail?.effect;
  if (effect?.effect === 'refresh') queue(effect.topics, false);
});

document.addEventListener('zeyn:topics', (e) => queue(e.detail?.topics, false));

document.addEventListener(
  'click',
  (e) => {
    const chip = e.target.closest?.('[data-zeyn-drives] a[data-zeyn-params]');
    if (!chip || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    e.preventDefault();
    let values = null;
    try {
      values = JSON.parse(chip.dataset.zeynParams);
    } catch {}
    if (values && typeof values === 'object') drive(chip.closest('[data-zeyn-drives]'), values, chip.href, true);
  },
  true,
);

document.addEventListener('change', (e) => {
  const form = e.target.closest?.('form[data-zeyn-drives]');
  if (form && !e.target.matches('[data-zeyn-date-input]')) drive(form, formValues(form));
});

document.addEventListener('input', (e) => {
  const form = e.target.closest?.('form[data-zeyn-drives]');
  if (form && e.target.matches('[data-zeyn-date-input]') && (e.target.dataset.iso || !e.target.value.trim())) drive(form, formValues(form));
});

document.addEventListener('submit', (e) => {
  const form = e.target.closest?.('form[data-zeyn-drives]');
  if (!form || form.matches('[data-zeyn-searchbox]')) return;
  e.preventDefault();
  drive(form, formValues(form));
});
