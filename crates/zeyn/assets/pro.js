/**
 * Zeyn desktop class shell.
 *
 * The command palette (Ctrl+Shift+P or F1): an ARIA combobox over a listbox that searches the
 * app's commands, the toolbar and context menu of the current screen and the screens of the menu.
 * Commands come from the server, which leaves out those the user may not run. Effects run
 * through the runtime (`zeyn:run`), so this script never builds requests the server would not
 * build itself. Interaction is wired with event delegation on `document`.
 * @module pro
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const MAC = /mac|iphone|ipad/i.test(navigator.userAgentData?.platform ?? navigator.platform ?? '');

/** Returns the CSRF token from `meta[name=zeyn-csrf]`. */
function csrf() {
  return document.querySelector('meta[name="zeyn-csrf"]')?.content ?? '';
}

/**
 * Posts JSON with the CSRF header.
 * @returns {Promise<{ok: boolean, status: number, data: any}>} Status 0 when unreachable.
 */
async function post(path, body) {
  try {
    const res = await fetch(BASE + path, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': csrf() },
      body: JSON.stringify(body),
    });
    let data = null;
    try {
      data = res.status === 204 ? null : await res.json();
    } catch {
      data = null;
    }
    return { ok: res.ok, status: res.status, data };
  } catch {
    return { ok: false, status: 0, data: null };
  }
}

/** Asks the runtime to run effects, a screen menu item or show an error (see `zeyn:run`). */
function run(detail) {
  document.dispatchEvent(new CustomEvent('zeyn:run', { detail }));
}

/** Creates an element with a class and attributes; `text` sets the text content. */
function make(tag, className, attrs = {}) {
  const el = document.createElement(tag);
  if (className) el.className = className;
  for (const [k, v] of Object.entries(attrs)) {
    if (k === 'text') el.textContent = v;
    else el.setAttribute(k, v);
  }
  return el;
}

/** Reads a stored JSON value; `fallback` when storage is blocked or the value is broken. */
function load(key, fallback) {
  try {
    return JSON.parse(localStorage.getItem(key) ?? 'null') ?? fallback;
  } catch {
    return fallback;
  }
}

/** Stores a JSON value; does nothing when storage is blocked or full. */
function store(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* private mode or full storage: the value is only kept for this page */
  }
}

/** Fills `{name}` placeholders. */
function fill(template, values) {
  return (template ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in values ? String(values[k]) : m));
}

/** A shortcut from `aria-keyshortcuts` as users read it. */
function shortcutText(keys) {
  return (keys ?? '').split(/\s+/)[0].replace(/Control/g, MAC ? '⌘' : 'Ctrl');
}

/* Command palette ---------------------------------------------------------------------------- */

const RECENT_KEY = 'zeyn.palette.recent';
const RECENT_MAX = 8;
const SHOWN_MAX = 60;

/** The open palette: `{dialog, input, list, live, items, shown, active, opener}`. */
let palette = null;

/** Palette texts from the server (`data-text-*` on `[data-zeyn-commands]`). */
function texts() {
  return document.querySelector('[data-zeyn-commands]')?.dataset ?? {};
}

/** Text for matching: lower case in the page language, without accents, dotless i as i. */
function fold(s) {
  return (s ?? '')
    .toLocaleLowerCase(document.documentElement.lang || undefined)
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .replace(/ı/g, 'i');
}

/**
 * Fuzzy match score of `query` in `text`: every query letter must appear in order. Letters in a
 * row and letters at the start of a word score more. Returns -1 when it does not match.
 */
function score(query, text) {
  const q = fold(query).replace(/\s+/g, '');
  if (!q) return 0;
  const t = fold(text);
  let qi = 0;
  let s = 0;
  let last = -2;
  for (let i = 0; i < t.length && qi < q.length; i++) {
    if (t[i] !== q[qi]) continue;
    s += 1;
    if (i === last + 1) s += 3;
    if (i === 0 || /[\s\-_./›:]/.test(t[i - 1])) s += 4;
    last = i;
    qi++;
  }
  if (qi < q.length) return -1;
  return s + (t.startsWith(q) ? 10 : 0) - t.length * 0.01;
}

/** The screen the user works in: the top window, or the page screen. */
function currentScreen() {
  const windows = [...document.querySelectorAll('dialog[open] [data-zeyn-screen]')].filter((s) => !s.closest('.z-palette'));
  return windows.at(-1) ?? document.querySelector('#z-main [data-zeyn-screen]');
}

/** Every command the palette offers, in a stable order. */
function collect() {
  const t = texts();
  const items = [];
  const keys = new Set();
  const add = (item) => {
    if (!item.label || keys.has(item.key)) return;
    keys.add(item.key);
    items.push(item);
  };
  for (const b of document.querySelectorAll('[data-zeyn-commands] > button[data-zeyn-command]')) {
    add({ key: `cmd:${b.dataset.zeynCommand}`, label: b.textContent.trim(), group: t.textApp, hint: b.dataset.hint, run: () => b.click() });
  }
  const screen = currentScreen();
  if (screen) {
    const name = screen.dataset.zeynScreen;
    const title = screen.querySelector('h1, h2')?.textContent.trim() || t.textScreen;
    for (const b of screen.querySelectorAll('.z-toolbar button:not([disabled]), [data-zeyn-menubar] button[data-zeyn-command]')) {
      const label = (b.querySelector('.z-menu-label')?.textContent ?? b.textContent).trim() || b.getAttribute('aria-label');
      const key = b.dataset.zeynCommand ? `cmd:${b.dataset.zeynCommand}` : `tb:${name}:${b.dataset.testid ?? label}`;
      add({ key, label, group: title, hint: shortcutText(b.getAttribute('aria-keyshortcuts')), run: () => b.click() });
    }
    let menu = [];
    try {
      menu = JSON.parse(screen.dataset.zeynMenu || '[]');
    } catch {
      menu = [];
    }
    const walk = (list, prefix) => {
      for (const i of list) {
        if (i.separator || i.disabled) continue;
        const label = prefix ? `${prefix} › ${i.label}` : i.label;
        if (i.items) walk(i.items, label);
        else if (i.effect || i.run != null) {
          add({
            key: `menu:${name}:${label}`,
            label,
            group: title,
            hint: i.hint,
            run: () => (i.effect ? run({ effects: [i.effect], source: screen }) : run({ screen, item: i.run })),
          });
        }
      }
    };
    walk(menu, '');
  }
  for (const a of document.querySelectorAll('.z-nav a[href]')) {
    const group = a.closest('.z-nav-group')?.querySelector('h2')?.textContent.trim();
    const path = new URL(a.href, location.href).pathname;
    add({ key: `nav:${path}`, label: a.textContent.trim(), group: group ? `${t.textGo} · ${group}` : t.textGo, run: () => a.click() });
  }
  return items;
}

/** Keys of recently run commands, newest first. */
function recent() {
  const list = load(RECENT_KEY, []);
  return Array.isArray(list) ? list.filter((k) => typeof k === 'string') : [];
}

/** Remembers a command as the most recent one. */
function remember(key) {
  store(RECENT_KEY, [key, ...recent().filter((k) => k !== key)].slice(0, RECENT_MAX));
}

/** Items matching the query: recent ones first, then by score, then in their own order. */
function rank(items, query) {
  const recents = recent();
  const scored = [];
  items.forEach((item, i) => {
    const own = score(query, item.label);
    const s = own >= 0 ? own : score(query, item.group ?? '') - 20;
    if (s < -20 || (own < 0 && s <= -20)) return;
    const r = recents.indexOf(item.key);
    scored.push({ item, s, r: r < 0 ? Infinity : r, i });
  });
  scored.sort((a, b) => {
    const ra = a.r !== Infinity;
    const rb = b.r !== Infinity;
    if (ra !== rb) return ra ? -1 : 1;
    if (ra && rb && !query) return a.r - b.r;
    return b.s - a.s || a.r - b.r || a.i - b.i;
  });
  return scored.slice(0, SHOWN_MAX).map((x) => ({ ...x.item, recent: x.r !== Infinity }));
}

/** Draws the options for the current query and announces how many there are. */
function drawPalette() {
  const p = palette;
  const t = texts();
  p.shown = rank(p.items, p.input.value);
  p.active = p.shown.length ? 0 : -1;
  p.list.replaceChildren();
  p.shown.forEach((item, i) => {
    const li = make('li', 'z-palette-option', { role: 'option', id: `z-palette-opt-${i}`, 'aria-selected': 'false' });
    li.append(make('span', 'z-palette-label', { text: item.label }));
    const group = item.recent ? `${t.textRecent} · ${item.group ?? ''}` : item.group;
    if (group) li.append(make('span', 'z-palette-group', { text: group }));
    if (item.hint) li.append(make('kbd', 'z-palette-kbd', { text: item.hint }));
    li.addEventListener('click', () => choose(i));
    li.addEventListener('pointermove', () => setActive(i, false));
    p.list.append(li);
  });
  if (!p.shown.length) p.list.append(make('li', 'z-palette-empty', { role: 'presentation', text: t.textEmpty }));
  setActive(p.active, true);
  p.live.textContent = p.shown.length ? fill(t.textCount, { n: p.shown.length }) : t.textEmpty;
}

/** Marks option `i` as active and keeps it in view. */
function setActive(i, scroll) {
  const p = palette;
  p.active = i;
  [...p.list.querySelectorAll('[role="option"]')].forEach((li, j) => li.setAttribute('aria-selected', String(j === i)));
  const li = p.list.querySelector(`#z-palette-opt-${i}`);
  if (li) {
    p.input.setAttribute('aria-activedescendant', li.id);
    if (scroll) li.scrollIntoView({ block: 'nearest' });
  } else p.input.removeAttribute('aria-activedescendant');
}

/** Runs option `i`: closes the palette (focus goes back to where it was), then runs it. */
function choose(i) {
  const item = palette?.shown[i];
  if (!item) return;
  remember(item.key);
  closePalette();
  item.run();
}

/** Opens the palette as a modal dialog. */
function openPalette() {
  if (palette) return;
  const t = texts();
  const dialog = make('dialog', 'z-palette', { 'aria-label': t.textLabel ?? 'Commands' });
  const input = make('input', 'z-palette-input', {
    type: 'text',
    role: 'combobox',
    'aria-expanded': 'true',
    'aria-controls': 'z-palette-list',
    'aria-autocomplete': 'list',
    'aria-label': t.textLabel ?? 'Commands',
    'aria-describedby': 'z-palette-hint',
    placeholder: t.textPlaceholder ?? '',
    autocomplete: 'off',
    spellcheck: 'false',
  });
  const list = make('ul', 'z-palette-list', { role: 'listbox', id: 'z-palette-list', 'aria-label': t.textLabel ?? 'Commands' });
  const live = make('p', 'z-sr-only', { 'aria-live': 'polite' });
  const hint = make('p', 'z-palette-hint', { id: 'z-palette-hint', text: t.textHint ?? '' });
  dialog.append(input, list, hint, live);
  document.body.append(dialog);
  palette = { dialog, input, list, live, items: collect(), shown: [], active: -1, opener: document.activeElement };
  dialog.addEventListener('close', () => {
    if (palette?.dialog === dialog) closePalette();
  });
  dialog.addEventListener('click', (e) => {
    if (e.target === dialog) closePalette();
  });
  input.addEventListener('input', drawPalette);
  input.addEventListener('keydown', paletteKey);
  dialog.showModal();
  drawPalette();
  input.focus();
}

/** Closes the palette right away and puts focus back where it was before it opened. */
function closePalette() {
  const p = palette;
  if (!p) return;
  palette = null;
  if (p.dialog.open) p.dialog.close();
  p.dialog.remove();
  if (p.opener instanceof HTMLElement && p.opener.isConnected) p.opener.focus();
}

/** Keys in the palette input: arrows move, Page keys jump, Enter runs. Escape is the dialog's. */
function paletteKey(e) {
  const p = palette;
  const n = p.shown.length;
  if (!n) return;
  const step = { ArrowDown: 1, ArrowUp: -1, PageDown: 8, PageUp: -8 }[e.key];
  if (step !== undefined) {
    e.preventDefault();
    const next = Math.abs(step) === 1 ? (p.active + step + n) % n : Math.max(0, Math.min(n - 1, p.active + step));
    setActive(next, true);
  } else if (e.key === 'Enter') {
    e.preventDefault();
    choose(p.active);
  }
}

/** True for the palette shortcut: Ctrl+Shift+P (Command on macOS) or F1. */
function paletteShortcut(e) {
  if (e.key === 'F1' && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) return true;
  return e.key.toLowerCase() === 'p' && e.shiftKey && !e.altKey && (MAC ? e.metaKey : e.ctrlKey);
}

/* Commands that run on the server ------------------------------------------------------------ */

/** Runs an action command with `POST /_zeyn/command`; the server checks the permission again. */
async function runCommand(button) {
  const r = await post('/_zeyn/command', { command: button.dataset.zeynCommandRun });
  if (!r.ok) {
    run({ error: r.data?.message || `Request failed (${r.status}).` });
    return;
  }
  run({ effects: [...(r.data?.effects ?? []), { effect: 'toast', message: texts().textDone ?? 'Done.' }], source: button });
}

/* Wiring ------------------------------------------------------------------------------------- */

document.addEventListener(
  'keydown',
  (e) => {
    if (e.defaultPrevented || e.repeat || !paletteShortcut(e)) return;
    e.preventDefault();
    e.stopPropagation();
    if (palette) closePalette();
    else openPalette();
  },
  true,
);

document.addEventListener('click', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t) return;
  let el;
  if ((el = t.closest('[data-zeyn-palette-open]'))) return openPalette();
  if ((el = t.closest('[data-zeyn-command-run]'))) return runCommand(el);
});
