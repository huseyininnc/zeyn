/**
 * DevTools of debug builds. The page links this script only while the server records requests
 * (`App::serve` in a debug build); release builds have neither the script nor its endpoint.
 * Alt+Shift+D opens a panel with the last requests from `GET /_zeyn/devtools`: the screen and
 * action of each, its status and time, how long the screen took to build and to render, every
 * SQL statement with its time, and the effects and topics the action returned. The list reloads
 * when new content or an effect arrives. Escape or Alt+Shift+D closes the panel and gives focus
 * back. Its stylesheet loads on first open. Everything is written with DOM calls, never as HTML.
 * @module devtools
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

let panel = null;
let list = null;
let back = null;
let timer = 0;

/** An element with a class and text. */
function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** Milliseconds as text. */
function ms(n) {
  return n == null ? '' : `${n} ms`;
}

/** Creates the panel and links the stylesheet once. */
function build() {
  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = `${BASE}/_zeyn/devtools.css${VERSION}`;
  document.head.append(link);
  const t = texts();
  panel = el('section', 'z-devtools');
  panel.setAttribute('aria-labelledby', 'z-devtools-title');
  const head = el('header', 'z-devtools-head');
  const title = el('h2', '', t['devtools.title'] ?? '');
  title.id = 'z-devtools-title';
  const refresh = el('button', 'z-devtools-button', t['devtools.refresh'] ?? '');
  refresh.type = 'button';
  refresh.addEventListener('click', load);
  const close = el('button', 'z-devtools-button', t['devtools.close'] ?? '');
  close.type = 'button';
  close.addEventListener('click', () => toggle(false));
  head.append(title, refresh, close);
  list = el('div', 'z-devtools-list');
  list.setAttribute('aria-live', 'polite');
  list.setAttribute('aria-busy', 'false');
  panel.append(head, el('p', 'z-devtools-hint', t['devtools.hint'] ?? ''), list);
}

/** The details of one request. */
function request(q, t) {
  const item = el('details', 'z-devtools-item');
  const call = [q.kind, q.screen, q.action].filter(Boolean).join(' ');
  const parts = [`${q.method} ${q.path}`, String(q.status), ms(q.ms), call];
  if (q.query_count) parts.push(`${q.query_count} SQL ${ms(q.query_ms)}`);
  const summary = el('summary', q.ok === false || q.status >= 400 ? 'z-devtools-failed' : '', parts.filter(Boolean).join(' · '));
  item.append(summary);
  const facts = el('dl', 'z-devtools-facts');
  const fact = (name, value) => {
    if (!value) return;
    facts.append(el('dt', '', name), el('dd', '', value));
  };
  fact(t['devtools.build'] ?? '', ms(q.build_ms));
  fact(t['devtools.render'] ?? '', ms(q.render_ms));
  fact(t['devtools.effects'] ?? '', (q.effects ?? []).join(', '));
  fact(t['devtools.topics'] ?? '', (q.topics ?? []).join(', '));
  item.append(facts);
  if (q.queries?.length) {
    item.append(el('h3', '', t['devtools.queries'] ?? ''));
    const ol = el('ol', 'z-devtools-queries');
    for (const s of q.queries) {
      const li = el('li');
      li.append(el('code', '', s.sql), el('span', 'z-devtools-ms', ` ${ms(s.ms)}${s.rows == null ? '' : ` · ${s.rows}`}`));
      ol.append(li);
    }
    item.append(ol);
  }
  return item;
}

/** Loads the recorded requests and lists them, keeping open items open. */
async function load() {
  if (!panel) return;
  list.setAttribute('aria-busy', 'true');
  try {
    const res = await fetch(`${BASE}/_zeyn/devtools`, { credentials: 'same-origin', headers: { accept: 'application/json' } });
    if (!res.ok) return;
    const { requests = [] } = await res.json();
    const t = texts();
    const open = new Set([...list.querySelectorAll('details[open] > summary')].map((s) => s.textContent));
    const items = requests.map((q) => request(q, t));
    for (const item of items) {
      if (open.has(item.querySelector('summary').textContent)) item.open = true;
    }
    list.replaceChildren(...(items.length ? items : [el('p', '', t['devtools.empty'] ?? '')]));
  } catch {
    return;
  } finally {
    list.setAttribute('aria-busy', 'false');
  }
}

/** Reloads the list shortly after the page changed, while the panel is open. */
function soon() {
  if (!panel?.isConnected) return;
  clearTimeout(timer);
  timer = setTimeout(load, 300);
}

/** Opens or closes the panel. */
function toggle(on) {
  if (on) {
    if (!panel) build();
    back = document.activeElement;
    document.body.append(panel);
    panel.querySelector('button').focus();
    load();
  } else if (panel?.isConnected) {
    clearTimeout(timer);
    panel.remove();
    if (back?.isConnected) back.focus();
  }
}

document.addEventListener('keydown', (e) => {
  if (e.altKey && e.shiftKey && !e.ctrlKey && !e.metaKey && e.code === 'KeyD') {
    e.preventDefault();
    toggle(!panel?.isConnected);
  } else if (e.key === 'Escape' && panel?.isConnected && panel.contains(document.activeElement)) {
    toggle(false);
  }
});
document.addEventListener('zeyn:content', soon);
document.addEventListener('zeyn:effect', soon);
