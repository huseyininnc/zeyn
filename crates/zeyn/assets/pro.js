/**
 * Zeyn desktop class shell.
 *
 * Menu bars, toolboxes, property inspector sections and status bars, and the command palette
 * (Ctrl+Shift+P or F1): an ARIA combobox over a listbox that searches the
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
async function runCommand(button, source = button) {
  const r = await post('/_zeyn/command', { command: button.dataset.zeynCommandRun });
  if (!r.ok) {
    run({ error: r.data?.message || `Request failed (${r.status}).` });
    return;
  }
  run({ effects: [...(r.data?.effects ?? []), { effect: 'toast', message: texts().textDone ?? 'Done.' }], source });
}

/* Menu bar (APG menubar) --------------------------------------------------------------------- */

/** Top level items of a menu bar. */
function topItems(bar) {
  return [...bar.querySelectorAll(':scope > li > .z-menubar-item')];
}

/** The menu a top level item opens. */
function menuOf(top) {
  return document.getElementById(top.getAttribute('aria-controls'));
}

/** The top level item a menu item belongs to. */
function topOf(item) {
  return item.closest('.z-menubar-top')?.querySelector(':scope > .z-menubar-item');
}

/** Items of an open menu, in order, without hidden ones. */
function itemsOf(menu) {
  return [...menu.querySelectorAll(':scope > li:not([hidden]) > [role^="menuitem"]')];
}

/** Whether any menu of the bar is open. */
function barOpen(bar) {
  return topItems(bar).some((t) => t.getAttribute('aria-expanded') === 'true');
}

/** Makes `top` the one top level item reached with Tab. */
function rove(bar, top) {
  for (const t of topItems(bar)) t.tabIndex = t === top ? 0 : -1;
}

/** Closes every menu of the bar except the one of `except`. */
function closeBarMenus(bar, except = null) {
  for (const t of topItems(bar)) {
    if (t === except) continue;
    t.setAttribute('aria-expanded', 'false');
    const menu = menuOf(t);
    if (menu) menu.hidden = true;
  }
}

/**
 * Opens the menu of a top level item. `focus` is `first`, `last` or `none` (the top item keeps
 * the focus, as when the pointer moves along the bar).
 */
function openBarMenu(top, focus = 'first') {
  const bar = top.closest('[data-zeyn-menubar]');
  closeBarMenus(bar, top);
  const menu = menuOf(top);
  document.dispatchEvent(new CustomEvent('zeyn:menu-open', { detail: menu }));
  menu.hidden = false;
  top.setAttribute('aria-expanded', 'true');
  rove(bar, top);
  const items = itemsOf(menu);
  const target = focus === 'last' ? items.at(-1) : focus === 'first' ? items[0] : top;
  (target ?? top).focus();
}

/** Visible text of a menu item, for type ahead. */
function itemText(el) {
  return (el.querySelector('.z-menu-label')?.textContent ?? el.textContent).trim();
}

/** The next item after `from` whose text starts with `ch`, wrapping around. */
function typeAhead(list, from, ch) {
  const start = list.indexOf(from);
  const c = fold(ch);
  for (let k = 1; k <= list.length; k++) {
    const el = list[(start + k) % list.length];
    if (fold(itemText(el)).startsWith(c)) return el;
  }
  return null;
}

/** Runs a menu item: closes the menus, puts focus on its top item, then runs the command. */
function activateBarItem(item) {
  if (item.getAttribute('aria-disabled') === 'true') return;
  if (item.getAttribute('role') === 'menuitemcheckbox') {
    document.dispatchEvent(new CustomEvent('zeyn:menu-check', { detail: item }));
    return;
  }
  const bar = item.closest('[data-zeyn-menubar]');
  const fromMenu = bar.contains(document.activeElement);
  const source = fromMenu ? topOf(item) : document.activeElement;
  closeBarMenus(bar);
  if (fromMenu) source?.focus();
  if (item.dataset.zeynCommandRun) runCommand(item, source);
  else if (item.dataset.zeynEffect) {
    try {
      run({ effects: [JSON.parse(item.dataset.zeynEffect)], source });
    } catch {
      /* a broken effect is never sent by the server */
    }
  }
}

/** Keys on a menu bar or in its menus. Returns true when handled. */
function barKey(e, t) {
  const bar = t.closest('[data-zeyn-menubar]');
  if (!bar) return false;
  const tops = topItems(bar);
  const rtl = getComputedStyle(bar).direction === 'rtl';
  const next = rtl ? 'ArrowLeft' : 'ArrowRight';
  const prev = rtl ? 'ArrowRight' : 'ArrowLeft';
  const printable = e.key.length === 1 && e.key !== ' ' && !e.ctrlKey && !e.metaKey && !e.altKey;
  if (t.matches('.z-menubar-item')) {
    const i = tops.indexOf(t);
    const open = barOpen(bar);
    const move = (target) => {
      rove(bar, target);
      if (open) openBarMenu(target, 'none');
      else target.focus();
    };
    if (e.key === next) move(tops[(i + 1) % tops.length]);
    else if (e.key === prev) move(tops[(i - 1 + tops.length) % tops.length]);
    else if (e.key === 'Home') move(tops[0]);
    else if (e.key === 'End') move(tops.at(-1));
    else if (e.key === 'ArrowDown' || e.key === 'Enter' || e.key === ' ') openBarMenu(t, 'first');
    else if (e.key === 'ArrowUp') openBarMenu(t, 'last');
    else if (e.key === 'Escape' && open) closeBarMenus(bar);
    else if (printable) {
      const hit = typeAhead(tops, t, e.key);
      if (hit) move(hit);
    } else return false;
    e.preventDefault();
    return true;
  }
  const menu = t.closest('.z-menubar-menu');
  if (!menu || !t.matches('[role^="menuitem"]')) return false;
  const items = itemsOf(menu);
  const i = items.indexOf(t);
  const top = topOf(t);
  const ti = tops.indexOf(top);
  if (e.key === 'ArrowDown') items[(i + 1) % items.length]?.focus();
  else if (e.key === 'ArrowUp') items[(i - 1 + items.length) % items.length]?.focus();
  else if (e.key === 'Home') items[0]?.focus();
  else if (e.key === 'End') items.at(-1)?.focus();
  else if (e.key === next) openBarMenu(tops[(ti + 1) % tops.length], 'first');
  else if (e.key === prev) openBarMenu(tops[(ti - 1 + tops.length) % tops.length], 'first');
  else if (e.key === 'Escape') {
    closeBarMenus(bar);
    top.focus();
  } else if (e.key === 'Enter' || e.key === ' ') activateBarItem(t);
  else if (e.key === 'Tab') {
    closeBarMenus(bar);
    return false;
  } else if (printable) typeAhead(items, t, e.key)?.focus();
  else return false;
  e.preventDefault();
  e.stopPropagation();
  return true;
}

/* Toolbox and status bar --------------------------------------------------------------------- */

/** Sets the live text of the status bar on the screen of `from`. */
function setStatus(from, text) {
  const scope = from?.closest?.('[data-zeyn-screen]') ?? document;
  const live = scope.querySelector('[data-zeyn-status-live]');
  if (live) live.textContent = text;
}

/** Turns one tool on and the others off, then tells the page and the status bar. */
function pickTool(button) {
  const box = button.closest('[data-zeyn-toolbox]');
  for (const b of box.querySelectorAll('.z-tool')) {
    const on = b === button;
    b.setAttribute('aria-pressed', String(on));
    b.tabIndex = on ? 0 : -1;
  }
  box.dispatchEvent(new CustomEvent('zeyn:tool', { bubbles: true, detail: { tool: button.dataset.tool } }));
  setStatus(box, fill(box.dataset.textSelected, { tool: button.getAttribute('aria-label') }));
}

/** Arrow keys, Home and End move between tools (roving tab stop). Returns true when handled. */
function toolKey(e, t) {
  const box = t.closest('[data-zeyn-toolbox]');
  const tools = [...box.querySelectorAll('.z-tool')];
  const i = tools.indexOf(t);
  const step = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[e.key];
  let target = null;
  if (step !== undefined) target = tools[(i + step + tools.length) % tools.length];
  else if (e.key === 'Home') target = tools[0];
  else if (e.key === 'End') target = tools.at(-1);
  else return false;
  e.preventDefault();
  for (const b of tools) b.tabIndex = b === target ? 0 : -1;
  target.focus();
  return true;
}

/* Wiring ------------------------------------------------------------------------------------- */

document.addEventListener(
  'keydown',
  (e) => {
    if (e.defaultPrevented) return;
    const t = e.target instanceof Element ? e.target : null;
    if (!e.repeat && paletteShortcut(e)) {
      e.preventDefault();
      e.stopPropagation();
      if (palette) closePalette();
      else openPalette();
      return;
    }
    if (!t) return;
    if (t.closest('[data-zeyn-menubar]') && barKey(e, t)) return;
    if (t.matches('[data-zeyn-toolbox] .z-tool')) toolKey(e, t);
  },
  true,
);

document.addEventListener(
  'click',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    const item = t?.closest('.z-menubar-menu [role^="menuitem"]');
    if (!item) return;
    e.preventDefault();
    e.stopPropagation();
    activateBarItem(item);
  },
  true,
);

document.addEventListener('click', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t) return;
  let el;
  if ((el = t.closest('[data-zeyn-palette-open]'))) return openPalette();
  if ((el = t.closest('[data-zeyn-command-run]'))) return runCommand(el);
  if ((el = t.closest('.z-menubar-item'))) {
    if (el.getAttribute('aria-expanded') === 'true') closeBarMenus(el.closest('[data-zeyn-menubar]'));
    else openBarMenu(el, 'first');
    return;
  }
  if ((el = t.closest('[data-zeyn-toolbox] .z-tool'))) return pickTool(el);
});

document.addEventListener('pointerover', (e) => {
  const top = e.target instanceof Element ? e.target.closest('.z-menubar-item') : null;
  const bar = top?.closest('[data-zeyn-menubar]');
  if (bar && barOpen(bar) && top.getAttribute('aria-expanded') !== 'true') openBarMenu(top, 'none');
});

document.addEventListener(
  'pointerdown',
  (e) => {
    const inside = e.target instanceof Element ? e.target.closest('[data-zeyn-menubar]') : null;
    for (const bar of document.querySelectorAll('[data-zeyn-menubar]')) if (bar !== inside && barOpen(bar)) closeBarMenus(bar);
  },
  true,
);

document.addEventListener('focusout', (e) => {
  const bar = e.target instanceof Element ? e.target.closest('[data-zeyn-menubar]') : null;
  if (bar && !(e.relatedTarget instanceof Element && bar.contains(e.relatedTarget)) && barOpen(bar)) closeBarMenus(bar);
});

document.addEventListener('zeyn:status', (e) => setStatus(e.target instanceof Element ? e.target : null, String(e.detail?.text ?? '')));

/** The browser's own checks run before a submit; open a folded section that holds a failing field. */
document.addEventListener(
  'invalid',
  (e) => {
    const d = e.target instanceof Element ? e.target.closest('details.z-form-section:not([open])') : null;
    if (d) d.open = true;
  },
  true,
);

/** A property inspector folds sections away; open the ones that hold fields with errors. */
document.addEventListener('zeyn:errors', (e) => {
  const form = e.target;
  if (!(form instanceof HTMLFormElement)) return;
  let first = null;
  for (const d of form.querySelectorAll('details.z-form-section')) {
    const bad = d.querySelector('[aria-invalid="true"]');
    if (!bad) continue;
    d.open = true;
    first ??= bad;
  }
  if (first && document.activeElement?.getAttribute('aria-invalid') !== 'true') first.focus();
});
