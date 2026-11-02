/**
 * Zeyn browser runtime.
 *
 * Implements sections 3, 4 and 6 of `docs/runtime-protocol.md`: effects, windows, grids,
 * forms, keyboard shortcuts, export and language selection, plus optional WebMCP tools.
 * Interaction is wired with event delegation on `document`, so screens loaded into dialogs
 * work without rebinding. Only grids need per element setup, done by `initGrids`.
 * @module zeyn
 */

/** Path the app is served under, from `meta[name=zeyn-base]`; empty when served at the root. */
const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const TOAST_MS = 4000;
const FILTER_DELAY_MS = 300;
const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([type="hidden"]):not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
const UNREACHABLE = 'The server could not be reached.';
const V_CAP = 8_000_000;
const V_OVERSCAN = 20;
const V_CACHE = 20;
const V_FETCH_DELAY_MS = 40;
const MAC = /mac|iphone|ipad/i.test(navigator.userAgentData?.platform ?? navigator.platform ?? '');

/** @type {WeakMap<Element, object>} Per grid root state. */
const grids = new WeakMap();
/** @type {{dialog: HTMLDialogElement, opener: Element | null}[]} Open windows, top last. */
const dialogs = [];
let uid = 0;

/** Returns the CSRF token from `meta[name=zeyn-csrf]`. */
function csrf() {
  return document.querySelector('meta[name="zeyn-csrf"]')?.content ?? '';
}

/**
 * Parses a JSON response body; returns `null` for empty or invalid bodies.
 * @param {Response} res
 */
async function readJson(res) {
  if (res.status === 204) return null;
  try {
    return await res.json();
  } catch {
    return null;
  }
}

/**
 * Sends a JSON `POST` with the CSRF header. Error responses show their `message` in an error
 * toast, except `422` (handled by the caller) and when `quiet` is set.
 * @param {string} url
 * @param {object} body
 * @param {{quiet?: boolean, signal?: AbortSignal}} [options] An aborted request resolves
 *   silently with status 0.
 * @returns {Promise<{status: number, ok: boolean, data: any}>}
 */
async function post(url, body, { quiet = false, signal } = {}) {
  let res, data;
  try {
    res = await fetch(url, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': csrf() },
      body: JSON.stringify(body),
      signal,
    });
    data = await readJson(res);
  } catch (err) {
    if (!quiet && err?.name !== 'AbortError') toast(UNREACHABLE, 'error');
    return { status: 0, ok: false, data: null };
  }
  if (!res.ok && res.status !== 422 && !quiet) toast(data?.message || `Request failed (${res.status}).`, 'error');
  return { status: res.status, ok: res.ok, data };
}

/** Builds the `?params=` query string; empty when the screen has no params. */
function paramsQuery(params) {
  return params == null ? '' : `?params=${encodeURIComponent(JSON.stringify(params))}`;
}

/** Returns the closest screen root of an element. */
function screenOf(el) {
  return el?.closest('[data-zeyn-screen]') ?? null;
}

/** Parses `data-zeyn-params` of a screen root. */
function paramsOf(screen) {
  const raw = screen?.dataset.zeynParams;
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

/** Returns the node index of the closest `[data-zeyn-node]`, or `null`. */
function nodeOf(el) {
  const node = el.closest('[data-zeyn-node]');
  return node ? Number(node.dataset.zeynNode) : null;
}

/** Common request fields: screen name, params and node of an element. */
function address(el) {
  const screen = screenOf(el);
  return { screen: screen?.dataset.zeynScreen ?? null, params: paramsOf(screen), node: nodeOf(el) };
}

/** Reads a numeric data attribute; absent, empty or `null` gives `null`. */
function num(value) {
  return value == null || value === '' || value === 'null' ? null : Number(value);
}

/** Reads a string data attribute; absent, empty or `null` gives `null`. */
function str(value) {
  return value == null || value === '' || value === 'null' ? null : value;
}

/** Formats a number for the page language. */
function fmt(n) {
  return new Intl.NumberFormat(document.documentElement.lang || undefined).format(n);
}

/** Replaces `{name}` placeholders in a localized template. */
function fill(template, values) {
  return template.replace(/\{(\w+)\}/g, (m, k) => (k in values ? String(values[k]) : m));
}

/** Returns the toast region of the top dialog, or of the page, creating it when missing. */
function toastRegion() {
  const host = dialogs.at(-1)?.dialog ?? document.body;
  let region = [...host.querySelectorAll('.z-toasts')].find((r) => r.closest('dialog') === (host === document.body ? null : host));
  if (!region) {
    region = document.createElement('div');
    region.className = 'z-toasts';
    region.setAttribute('role', 'status');
    region.setAttribute('aria-live', 'polite');
    host.append(region);
  }
  return region;
}

/**
 * Shows a toast for four seconds. The timer pauses while the pointer is over the toast or it
 * holds focus (WCAG 2.2.1).
 * @param {string} message
 * @param {'info' | 'error'} [variant]
 */
function toast(message, variant = 'info') {
  const el = document.createElement('div');
  el.className = `z-toast z-toast-${variant}`;
  el.textContent = message;
  let left = TOAST_MS;
  let started = 0;
  let timer = 0;
  const run = () => {
    if (timer || el.matches(':hover, :focus-within')) return;
    started = Date.now();
    timer = setTimeout(() => el.remove(), left);
  };
  const pause = () => {
    if (!timer) return;
    clearTimeout(timer);
    timer = 0;
    left = Math.max(1000, left - (Date.now() - started));
  };
  el.addEventListener('pointerenter', pause);
  el.addEventListener('focusin', pause);
  el.addEventListener('pointerleave', () => setTimeout(run));
  el.addEventListener('focusout', () => setTimeout(run));
  toastRegion().append(el);
  run();
}

/** Parses the JSON effect of an element. */
function parseEffect(el) {
  try {
    return JSON.parse(el.dataset.zeynEffect);
  } catch {
    return null;
  }
}

/**
 * Runs one effect.
 * @param {{effect: string, screen?: string, params?: any, message?: string}} effect
 * @param {Element} [source] Element that triggered it; focus returns there when a window closes.
 */
async function runEffect(effect, source) {
  switch (effect?.effect) {
    case 'open_window':
      return openWindow(effect.screen, effect.params, source);
    case 'navigate':
      location.assign(`${BASE}/_zeyn/route/${encodeURIComponent(effect.screen)}${paramsQuery(effect.params)}`);
      return;
    case 'toast':
      return toast(effect.message ?? '');
    case 'close_and_refresh':
      return closeAndRefresh();
  }
}

/** Runs effects in order. */
async function runEffects(effects, source) {
  for (const effect of effects ?? []) await runEffect(effect, source);
}

/**
 * Loads a screen fragment into a modal `<dialog>` on top of the stack.
 * The native dialog carries the dialog role, so the role is removed from the inner window.
 */
async function openWindow(screen, params, opener) {
  let res;
  try {
    res = await fetch(`${BASE}/_zeyn/screen/${encodeURIComponent(screen)}${paramsQuery(params)}`, {
      credentials: 'same-origin',
      headers: { accept: 'text/html' },
    });
  } catch {
    return toast(UNREACHABLE, 'error');
  }
  if (!res.ok) {
    const data = await readJson(res);
    return toast(data?.message || `Request failed (${res.status}).`, 'error');
  }
  const dialog = document.createElement('dialog');
  dialog.className = 'z-dialog';
  dialog.innerHTML = await res.text();
  const win = dialog.querySelector('[data-zeyn-screen]');
  if (win) {
    win.removeAttribute('role');
    win.removeAttribute('aria-modal');
    win.tabIndex = -1;
    if (!win.hasAttribute('data-zeyn-params')) win.dataset.zeynParams = JSON.stringify(params ?? null);
    const label = win.getAttribute('aria-labelledby');
    if (label) dialog.setAttribute('aria-labelledby', label);
  }
  const close = document.createElement('button');
  close.type = 'button';
  close.className = 'z-close';
  close.dataset.zeynClose = '';
  close.setAttribute('aria-label', 'Close');
  close.textContent = '×';
  dialog.append(close);
  document.body.append(dialog);
  dialogs.push({ dialog, opener: opener ?? document.activeElement });
  dialog.addEventListener('close', () => finishDialog(dialog));
  dialog.showModal();
  initGrids(dialog);
  (win?.querySelector(FOCUSABLE) ?? win ?? close).focus();
  syncAgentTools();
}

/**
 * Removes a dialog from the stack right away, closes it and returns focus to its opener.
 * Runs again from the asynchronous `close` event, where it does nothing.
 */
function finishDialog(dialog) {
  const i = dialogs.findIndex((d) => d.dialog === dialog);
  if (i < 0) return;
  const [entry] = dialogs.splice(i, 1);
  if (dialog.open) dialog.close();
  dialog.remove();
  if (entry.opener?.isConnected) entry.opener.focus();
  syncAgentTools();
}

/** Closes the top dialog and reloads every grid of the screen under it. */
async function closeAndRefresh() {
  const under = dialogs.at(-2)?.dialog ?? document.querySelector('.z-main') ?? document.body;
  const top = dialogs.at(-1);
  if (top) finishDialog(top.dialog);
  await Promise.all([...under.querySelectorAll('[data-zeyn-grid]')].map((g) => loadGrid(g, 'reload')));
}

/** Rows currently in a grid. */
function rows(grid) {
  return [...(grid.querySelector('[data-zeyn-rows]')?.rows ?? [])];
}

/** Sets up every grid inside `root`. Safe to call more than once. */
function initGrids(root) {
  root.querySelectorAll('[data-zeyn-grid]').forEach(initGrid);
}

/**
 * Sets up one grid: state from the server rendered first page, sort buttons, filter row,
 * infinite scroll observer and roving tabindex. Does not fetch.
 */
function initGrid(grid) {
  if (grids.has(grid)) return;
  const d = grid.dataset;
  const sorted = grid.querySelector('thead th[aria-sort="ascending"], thead th[aria-sort="descending"]');
  const state = {
    paging: d.zeynPaging || 'pages',
    size: Number(d.zeynPageSize) || 50,
    offset: num(d.zeynOffset) ?? 0,
    total: num(d.zeynTotal),
    next: str(d.zeynNext),
    count: rows(grid).length,
    sort: sorted ? { field: sorted.dataset.field, desc: sorted.getAttribute('aria-sort') === 'descending' } : null,
    filters: {},
    seq: 0,
    timer: 0,
    active: 0,
    loading: false,
    observer: null,
  };
  grids.set(grid, state);

  const head = grid.querySelector('thead tr');
  for (const th of head?.cells ?? []) {
    if (!th.dataset.field) continue;
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'z-sort';
    button.append(...th.childNodes);
    th.append(button);
  }
  if (head && grid.querySelector('thead th[data-filter="true"]')) {
    const row = document.createElement('tr');
    row.className = 'z-filter-row';
    for (const th of head.cells) {
      const cell = document.createElement('td');
      if (th.dataset.filter === 'true') {
        const input = document.createElement('input');
        input.type = 'search';
        input.className = 'z-filter';
        input.id = `z-filter-${++uid}`;
        input.autocomplete = 'off';
        input.dataset.zeynFilter = th.dataset.field;
        input.setAttribute('aria-label', `Filter ${th.textContent.trim()}`);
        cell.append(input);
      }
      row.append(cell);
    }
    head.after(row);
  }

  if (state.paging === 'virtual') return initVirtual(grid, state);
  const more = grid.querySelector('[data-zeyn-more]');
  if (state.paging === 'infinite' && more && 'IntersectionObserver' in window) {
    state.observer = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) loadGrid(grid, 'more');
    });
  }
  roving(grid, false);
  updateChrome(grid);
}

/**
 * Loads rows from `POST /_zeyn/grid`.
 * @param {Element} grid Grid root.
 * @param {'reset' | 'reload' | 'prev' | 'next' | 'more'} kind `reset` starts over (sort, filter),
 *   `reload` keeps the current page, `more` appends the next infinite batch.
 */
async function loadGrid(grid, kind) {
  if (!grid) return;
  initGrid(grid);
  const s = grids.get(grid);
  if (s.paging === 'virtual') return resetVirtual(grid, s, kind);
  const infinite = s.paging === 'infinite';
  const append = kind === 'more';
  if (append && (s.loading || s.next == null)) return;
  let offset = s.offset;
  if (kind === 'reset' || infinite) offset = 0;
  else if (kind === 'next') offset = s.next != null && /^\d+$/.test(s.next) ? Number(s.next) : s.offset + s.count;
  else if (kind === 'prev') offset = Math.max(0, s.offset - s.size);
  const page = infinite
    ? { type: 'after', cursor: append ? s.next : null, limit: s.size }
    : { type: 'offset', offset, limit: s.size };
  const filters = Object.entries(s.filters)
    .filter(([, v]) => v)
    .map(([field, contains]) => ({ field, contains }));
  const seq = ++s.seq;
  s.loading = true;
  grid.setAttribute('aria-busy', 'true');
  const r = await post(BASE + '/_zeyn/grid', {
    ...address(grid),
    query: { page, sort: s.sort ? [s.sort] : [], filters, with_total: !infinite },
  });
  if (seq !== s.seq) return;
  s.loading = false;
  grid.removeAttribute('aria-busy');
  if (!r.ok || !r.data) return;

  const body = grid.querySelector('[data-zeyn-rows]');
  const hadFocus = body.contains(document.activeElement);
  if (append) body.insertAdjacentHTML('beforeend', r.data.rows ?? '');
  else {
    body.innerHTML = r.data.rows ?? '';
    if (kind !== 'reload') s.active = 0;
  }
  s.offset = offset;
  s.count = body.rows.length;
  s.total = r.data.total ?? null;
  s.next = r.data.count === 0 && append ? null : (r.data.next ?? null);
  roving(grid, hadFocus);
  updateChrome(grid, append);
}

/** Applies the roving tabindex: the active row gets 0, the others -1. */
function roving(grid, focus) {
  const s = grids.get(grid);
  const list = rows(grid);
  s.active = Math.min(s.active, Math.max(0, list.length - 1));
  list.forEach((tr, i) => (tr.tabIndex = i === s.active ? 0 : -1));
  if (focus) list[s.active]?.focus();
}

/** Makes a row the active one and optionally focuses it. */
function activate(tr, focus) {
  const list = [...tr.parentElement.rows];
  list.forEach((r) => (r.tabIndex = r === tr ? 0 : -1));
  const s = grids.get(tr.closest('[data-zeyn-grid]'));
  if (s) s.active = tr.dataset.zeynIndex != null ? Number(tr.dataset.zeynIndex) : list.indexOf(tr);
  if (focus) tr.focus();
}

/** Updates pager buttons, page info, the "Load more" button and the scroll observer. */
function updateChrome(grid, announce = false) {
  const s = grids.get(grid);
  const d = grid.dataset;
  const info = grid.querySelector('[data-zeyn-page-info]');
  if (s.paging === 'pages') {
    const prev = grid.querySelector('[data-zeyn-page="prev"]');
    const next = grid.querySelector('[data-zeyn-page="next"]');
    const hasNext = s.total != null ? s.offset + s.count < s.total : s.next != null;
    const focused = document.activeElement;
    if (prev) prev.disabled = s.offset <= 0;
    if (next) next.disabled = !hasNext;
    if (focused === prev && prev.disabled) next?.focus();
    if (focused === next && next.disabled) prev?.focus();
    if (info) {
      const values = { from: fmt(s.count ? s.offset + 1 : 0), to: fmt(s.offset + s.count), total: fmt(s.total ?? 0) };
      const template = s.total != null ? d.zeynTextRangeTotal || '{from}-{to} of {total}' : d.zeynTextRange || '{from}-{to}';
      info.textContent = fill(template, values);
    }
  } else if (s.paging === 'infinite') {
    const more = grid.querySelector('[data-zeyn-more]');
    const button = more?.querySelector('button');
    if (button) button.hidden = s.next == null;
    if (info && announce) info.textContent = fill(d.zeynTextLoaded || '{count} rows loaded', { count: fmt(s.count) });
    if (s.observer && more) {
      s.observer.unobserve(more);
      if (s.next != null) s.observer.observe(more);
    }
  }
}

/**
 * Sets up a virtual grid (protocol 4.6). The grid root is the scroll container; a spacer
 * `tbody` after the rows gives the table its full height (capped at 8,000,000 px), and the rows
 * `tbody` holds only the visible rows plus 20 above and below, moved with `translateY`.
 * The server rendered first block becomes the first cache entry.
 */
function initVirtual(grid, s) {
  const body = grid.querySelector('[data-zeyn-rows]');
  const head = grid.querySelector('thead');
  const columns = head.rows[0]?.cells.length ?? 1;
  const spacer = document.createElement('tbody');
  spacer.className = 'z-spacer';
  spacer.setAttribute('aria-hidden', 'true');
  spacer.innerHTML = `<tr><td colspan="${columns}"></td></tr>`;
  body.after(spacer);
  const placeholder = document.createElement('tr');
  placeholder.className = 'z-placeholder';
  placeholder.setAttribute('aria-busy', 'true');
  placeholder.innerHTML = '<td></td>'.repeat(columns);
  Object.assign(s, {
    rh: Number(grid.dataset.zeynRowHeight) || 32,
    body,
    head,
    foot: grid.querySelector('tfoot'),
    table: grid.querySelector('table'),
    filler: spacer.rows[0].cells[0],
    placeholder,
    cache: new Map(),
    stale: new Map(),
    inflight: new Map(),
    failed: new Set(),
    holders: new Map(),
    gen: 0,
    version: 0,
    key: '',
    needTotal: s.total == null,
    exact: 0,
    dir: 1,
    view: 0,
    height: 0,
    fillerHeight: -1,
    rowCount: -1,
    raf: 0,
    fetchTimer: 0,
    infoTimer: 0,
  });
  const first = Math.floor(s.offset / s.size);
  s.cache.set(first, [...body.rows].map((tr, k) => prepRow(s, tr, first * s.size + k)));
  s.total ??= body.rows.length;
  grid.tabIndex = -1;
  measureVirtual(s);
  grid.addEventListener('scroll', () => scheduleVirtual(grid), { passive: true });
  new ResizeObserver(() => {
    measureVirtual(s);
    scheduleVirtual(grid);
  }).observe(grid);
  renderVirtual(grid);
}

/** Reads the sticky header and footer heights; called on setup and on resize only. */
function measureVirtual(s) {
  s.headH = s.head.offsetHeight;
  s.footH = s.foot?.offsetHeight ?? 0;
}

/** Marks a row with its absolute index, `aria-rowindex` and zebra parity. */
function prepRow(s, tr, i) {
  tr.dataset.zeynIndex = i;
  tr.setAttribute('aria-rowindex', s.head.rows.length + i + 1);
  tr.tabIndex = -1;
  tr.classList.toggle('z-alt', i % 2 === 1);
  return tr;
}

/** Schedules one render on the next animation frame. */
function scheduleVirtual(grid) {
  const s = grids.get(grid);
  if (!s.raf) s.raf = requestAnimationFrame(() => renderVirtual(grid));
}

/**
 * Returns the row for index `i` from the cache, the stale cache or a placeholder. Placeholders
 * that left the window are reused from `spare` instead of cloned.
 */
function virtualRow(s, i, spare) {
  const b = Math.floor(i / s.size);
  const tr = (s.cache.get(b) ?? s.stale.get(b))?.[i - b * s.size];
  if (tr) return tr;
  let holder = s.holders.get(i);
  if (!holder) {
    holder = prepRow(s, spare.pop() ?? s.placeholder.cloneNode(true), i);
    s.holders.set(i, holder);
  }
  return holder;
}

/**
 * Renders the visible window. Reads scroll geometry first, then writes; touches the DOM rows
 * only when the window or the loaded blocks changed. Focus stays on the focused row, or moves to
 * `focusIndex` when given.
 */
function renderVirtual(grid, focusIndex = null) {
  const s = grids.get(grid);
  s.raf = 0;
  const top = grid.scrollTop;
  const view = Math.max(s.rh, grid.clientHeight - s.headH - s.footH);
  const total = s.total ?? 0;
  const full = total * s.rh;
  const height = Math.min(full, V_CAP);
  const visible = view / s.rh;
  const range = Math.max(1, height - view);
  const exact = full <= V_CAP ? top / s.rh : (Math.min(top, range) / range) * Math.max(0, total - visible);
  const start = Math.max(0, Math.floor(exact) - V_OVERSCAN);
  const shift = top - (exact - start) * s.rh;
  const fit = Math.max(0, Math.floor((height - shift) / s.rh + 0.01));
  const end = Math.min(total, Math.ceil(exact + visible) + V_OVERSCAN, start + fit);
  if (exact !== s.exact) s.dir = exact > s.exact ? 1 : -1;
  Object.assign(s, { exact, view, height });

  s.body.style.transform = `translateY(${shift}px)`;
  const key = `${start}:${end}:${s.version}:${s.gen}`;
  if (key !== s.key) {
    s.key = key;
    const focused =
      focusIndex ?? (s.body.contains(document.activeElement) ? document.activeElement.dataset.zeynIndex : null);
    const spare = [];
    for (const [i, holder] of s.holders) {
      if (i < start || i >= end) {
        s.holders.delete(i);
        spare.push(holder);
      }
    }
    const nodes = [];
    for (let i = start; i < end; i++) nodes.push(virtualRow(s, i, spare));
    const roving = nodes.find((n) => Number(n.dataset.zeynIndex) === s.active) ?? nodes[0];
    for (const n of nodes) if (n.tabIndex !== (n === roving ? 0 : -1)) n.tabIndex = n === roving ? 0 : -1;
    const want = new Set(nodes);
    const active = document.activeElement;
    const kept = s.body.contains(active) && want.has(active);
    if (s.body.contains(active) && !kept) grid.focus({ preventScroll: true });
    for (const n of [...s.body.children]) if (!want.has(n)) n.remove();
    let ref = s.body.firstChild;
    for (const n of nodes) {
      if (n === ref) ref = ref.nextSibling;
      else s.body.insertBefore(n, ref);
    }
    const filler = Math.max(0, height - (end - start) * s.rh);
    if (filler !== s.fillerHeight) s.filler.style.height = `${(s.fillerHeight = filler)}px`;
    const target = focused != null && s.body.querySelector(`[data-zeyn-index="${focused}"]`);
    if (focused != null && !(kept && target === active)) (target || grid).focus({ preventScroll: true });
    const rowCount = s.head.rows.length + total;
    if (rowCount !== s.rowCount) s.table.setAttribute('aria-rowcount', (s.rowCount = rowCount));
    for (let b = Math.floor(start / s.size); b * s.size < end; b++) {
      const block = s.cache.get(b);
      if (block) {
        s.cache.delete(b);
        s.cache.set(b, block);
      }
    }
  }

  const blocks = new Set();
  for (let b = Math.floor(start / s.size); b * s.size < Math.max(end, start + 1); b++) blocks.add(b);
  const ahead = s.dir > 0 ? Math.max(...blocks) + 1 : Math.min(...blocks) - 1;
  if (ahead >= 0 && ahead * s.size < total) blocks.add(ahead);
  for (const [b, controller] of s.inflight) {
    if (!blocks.has(b)) {
      controller.abort();
      s.inflight.delete(b);
    }
  }
  clearTimeout(s.fetchTimer);
  s.fetchTimer = setTimeout(() => {
    for (const b of blocks) if (!s.cache.has(b) && !s.inflight.has(b) && !s.failed.has(b)) fetchBlock(grid, s, b);
  }, V_FETCH_DELAY_MS);

  clearTimeout(s.infoTimer);
  s.infoTimer = setTimeout(() => {
    const info = grid.querySelector('[data-zeyn-page-info]');
    if (!info) return;
    const from = Math.min(total, Math.floor(exact) + 1);
    const to = Math.min(total, Math.floor(exact + visible));
    info.textContent = fill(grid.dataset.zeynTextRangeTotal || '{from}-{to} of {total}', {
      from: fmt(from),
      to: fmt(to),
      total: fmt(total),
    });
  }, 400);
}

/** Fetches one block by offset; stale or aborted answers are dropped. */
async function fetchBlock(grid, s, b) {
  const controller = new AbortController();
  s.inflight.set(b, controller);
  const gen = s.gen;
  const withTotal = s.needTotal;
  const filters = Object.entries(s.filters)
    .filter(([, v]) => v)
    .map(([field, contains]) => ({ field, contains }));
  const page = { type: 'offset', offset: b * s.size, limit: s.size };
  const r = await post(
    BASE + '/_zeyn/grid',
    { ...address(grid), query: { page, sort: s.sort ? [s.sort] : [], filters, with_total: withTotal } },
    { signal: controller.signal },
  );
  if (s.inflight.get(b) === controller) s.inflight.delete(b);
  if (gen !== s.gen || controller.signal.aborted) return;
  if (!r.ok || !r.data) {
    s.failed.add(b);
    return;
  }
  const t = document.createElement('template');
  t.innerHTML = `<table><tbody>${r.data.rows ?? ''}</tbody></table>`;
  s.cache.set(b, [...t.content.querySelectorAll('tr')].map((tr, k) => prepRow(s, tr, b * s.size + k)));
  s.stale.delete(b);
  while (s.cache.size > V_CACHE) s.cache.delete(s.cache.keys().next().value);
  if (withTotal && r.data.total != null) {
    s.total = r.data.total;
    s.needTotal = false;
  }
  s.version++;
  scheduleVirtual(grid);
}

/**
 * Drops the cache of a virtual grid. Sort and filter changes scroll to the top; `reload`
 * keeps the position and shows the old rows until fresh blocks arrive.
 */
function resetVirtual(grid, s, kind) {
  for (const controller of s.inflight.values()) controller.abort();
  s.inflight.clear();
  s.failed.clear();
  s.holders.clear();
  s.stale = kind === 'reload' ? new Map([...s.stale, ...s.cache]) : new Map();
  s.cache = new Map();
  s.gen++;
  s.needTotal = true;
  if (kind !== 'reload') {
    s.active = 0;
    grid.scrollTop = 0;
  }
  renderVirtual(grid);
}

/** Handles Arrow, Page Up, Page Down, Home and End in a virtual grid. Returns true when handled. */
function virtualKey(e, grid) {
  const s = grids.get(grid);
  const total = s?.total ?? 0;
  if (!total) return false;
  const page = Math.max(1, Math.floor(s.view / s.rh) - 1);
  const step = { ArrowDown: 1, ArrowUp: -1, PageDown: page, PageUp: -page }[e.key];
  let i;
  if (step !== undefined) i = s.active + step;
  else if (e.key === 'Home') i = 0;
  else if (e.key === 'End') i = total - 1;
  else return false;
  e.preventDefault();
  renderVirtual(grid);
  s.active = Math.max(0, Math.min(total - 1, i));
  const visible = s.view / s.rh;
  let target = null;
  if (s.active < s.exact) target = s.active;
  else if (s.active + 1 > s.exact + visible) target = s.active + 1 - visible;
  if (target !== null) {
    const full = total * s.rh;
    const round = target === s.active ? Math.floor : Math.ceil;
    grid.scrollTop =
      full <= V_CAP ? target * s.rh : round((target / Math.max(1, total - visible)) * Math.max(1, s.height - s.view));
  }
  s.key = '';
  renderVirtual(grid, s.active);
  return true;
}

/** Cycles the sort of a column: none, ascending, descending. */
function toggleSort(button) {
  const th = button.closest('th');
  const grid = th.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  if (!s) return;
  const order = { none: 'ascending', ascending: 'descending', descending: 'none' };
  const next = order[th.getAttribute('aria-sort')] ?? 'ascending';
  grid.querySelectorAll('thead th[aria-sort]').forEach((h) => h.setAttribute('aria-sort', 'none'));
  th.setAttribute('aria-sort', next);
  s.sort = next === 'none' ? null : { field: th.dataset.field, desc: next === 'descending' };
  loadGrid(grid, 'reset');
}

/** Debounces a filter box and reloads the grid from the start. */
function onFilter(input) {
  const grid = input.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  if (!s) return;
  clearTimeout(s.timer);
  s.timer = setTimeout(() => {
    s.filters[input.dataset.zeynFilter] = input.value.trim();
    loadGrid(grid, 'reset');
  }, FILTER_DELAY_MS);
}

/** Handles arrow keys, Home, End and Enter on a grid row. Returns true when handled. */
function rowKey(e, tr) {
  const list = [...tr.parentElement.rows];
  const i = list.indexOf(tr);
  const target = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: list.length - 1 }[e.key];
  if (target !== undefined) {
    e.preventDefault();
    if (list[target]) activate(list[target], true);
    return true;
  }
  if (e.key === 'Enter' && tr.dataset.zeynEffect) {
    e.preventDefault();
    const effect = parseEffect(tr);
    if (effect) runEffect(effect, tr);
    return true;
  }
  return false;
}

/** True when the event target is a place where the user types text. */
function typing(target) {
  if (!(target instanceof Element)) return false;
  if (target.isContentEditable || target.closest('textarea, select')) return true;
  return target.matches('input') && !/^(checkbox|radio|button|submit|reset)$/.test(target.type);
}

/**
 * True when a key event matches one `aria-keyshortcuts` entry such as `Control+N`.
 * On macOS, Command (Meta) is accepted where Control is declared.
 */
function matchesShortcut(e, combo) {
  const parts = combo.split('+');
  const key = parts.pop().toLowerCase();
  const mods = new Set(parts.map((p) => p.toLowerCase()));
  const swap = MAC && mods.has('control') && !mods.has('meta');
  const ctrlMeta = swap
    ? e.ctrlKey !== e.metaKey
    : e.ctrlKey === mods.has('control') && e.metaKey === mods.has('meta');
  return e.key.toLowerCase() === key && ctrlMeta && e.shiftKey === mods.has('shift') && e.altKey === mods.has('alt');
}

/** Triggers the element whose `aria-keyshortcuts` matches, searching only the top dialog when one is open. */
function shortcut(e) {
  if (e.defaultPrevented || e.repeat || typing(e.target)) return;
  const scope = dialogs.at(-1)?.dialog ?? document;
  for (const el of scope.querySelectorAll('[aria-keyshortcuts]')) {
    if (el.disabled || (scope === document && el.closest('dialog'))) continue;
    if (el.getAttribute('aria-keyshortcuts').split(/\s+/).some((k) => k && matchesShortcut(e, k))) {
      e.preventDefault();
      el.click();
      return;
    }
  }
}

/**
 * Collects form values: booleans for checkboxes, strings otherwise. Skips read only inputs and
 * hidden inputs, except row versions (`data-zeyn-version`) used for conflict detection.
 */
function formValues(form) {
  const values = {};
  for (const el of form.elements) {
    if (!el.name || el.disabled || el.readOnly) continue;
    if (el.type === 'hidden' && !el.hasAttribute('data-zeyn-version')) continue;
    if (el.matches('button, fieldset, output, [type="submit"], [type="button"], [type="reset"]')) continue;
    if (el.type === 'checkbox') values[el.name] = el.checked;
    else if (el.type === 'radio') {
      if (el.checked) values[el.name] = el.value;
    } else values[el.name] = el.value;
  }
  return values;
}

/** Removes validation messages and `aria-invalid` from a form. */
function clearErrors(form) {
  form.querySelectorAll('.z-field-error').forEach((p) => (p.textContent = ''));
  form.querySelectorAll('[aria-invalid]').forEach((el) => el.removeAttribute('aria-invalid'));
  form.querySelector('.z-form-errors')?.remove();
}

/** Shows `422` errors under their fields, general errors on top, and focuses the first one. */
function showErrors(form, errors) {
  const scope = screenOf(form)?.dataset.zeynScreen;
  const general = [];
  let first = null;
  for (const err of errors) {
    const named = err.field ? form.elements.namedItem(err.field) : null;
    const input = named instanceof RadioNodeList ? named[0] : named;
    const id = input?.id ? `${input.id}-error` : `${scope}-${err.field}-error`;
    const box = input && form.querySelector(`#${CSS.escape(id)}`);
    if (!box) {
      general.push(err.message);
      continue;
    }
    box.textContent = box.textContent ? `${box.textContent} ${err.message}` : err.message;
    input.setAttribute('aria-invalid', 'true');
    first ??= input;
  }
  if (general.length) {
    const box = document.createElement('div');
    box.className = 'z-form-errors';
    box.setAttribute('role', 'alert');
    box.tabIndex = -1;
    for (const message of general) {
      const p = document.createElement('p');
      p.textContent = message;
      box.append(p);
    }
    form.prepend(box);
    first ??= box;
  }
  first?.focus();
}

/** Validates in the browser, posts to `/_zeyn/form`, then shows errors or runs effects. */
async function submitForm(form, submitter) {
  if (form.getAttribute('aria-busy') === 'true') return;
  clearErrors(form);
  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }
  const button = submitter ?? form.querySelector('[type="submit"]');
  if (button) button.disabled = true;
  form.setAttribute('aria-busy', 'true');
  try {
    const r = await post(BASE + '/_zeyn/form', { ...address(form), values: formValues(form) });
    if (r.status === 422) showErrors(form, r.data?.errors ?? []);
    else if (r.ok) await runEffects(r.data?.effects, form);
  } finally {
    if (button) button.disabled = false;
    form.removeAttribute('aria-busy');
  }
}

/** Quotes one CSV cell (RFC 4180) and defuses spreadsheet formulas in non numeric text. */
function csvCell(value) {
  let v = value;
  if (/^[=+\-@\t\r]/.test(v) && !/^[+-]?[\d.,\s]+$/.test(v)) v = `'${v}`;
  return /[",\r\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v;
}

/** Downloads the rows loaded in the screen's first grid as CSV; XLSX is not available yet. */
function exportGrid(button) {
  if (button.dataset.zeynExport === 'xlsx') return toast('XLSX export is not available yet.');
  const screen = screenOf(button);
  const grid = screen?.querySelector('[data-zeyn-grid]');
  if (!grid) return;
  const head = [...grid.querySelectorAll('thead tr:first-child th')].map((th) => th.textContent.trim());
  const s = grids.get(grid);
  const loaded =
    s?.paging === 'virtual' ? [...s.cache.keys()].sort((a, b) => a - b).flatMap((b) => s.cache.get(b)) : rows(grid);
  const body = loaded.map((tr) => [...tr.cells].map((td) => td.textContent.trim()));
  const csv = [head, ...body].map((r) => r.map(csvCell).join(',')).join('\r\n') + '\r\n';
  const url = URL.createObjectURL(new Blob(['﻿', csv], { type: 'text/csv;charset=utf-8' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = `${screen.dataset.zeynScreen || 'export'}.csv`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Stores the chosen language with `POST /_zeyn/locale` and reloads the page. */
async function changeLocale(select) {
  const r = await post(BASE + '/_zeyn/locale', { locale: select.value });
  if (r.ok) location.reload();
}

/** Stores a hand picked time zone (`""` = automatic) with `POST /_zeyn/timezone` and reloads. */
async function changeTimeZone(select) {
  const r = await post(BASE + '/_zeyn/timezone', { zone: select.value });
  if (r.ok) location.reload();
}

/** Reads the page cookies into an object. */
function cookies() {
  const out = {};
  for (const part of document.cookie.split(';')) {
    const i = part.indexOf('=');
    if (i < 0) continue;
    try {
      out[part.slice(0, i).trim()] = decodeURIComponent(part.slice(i + 1).trim());
    } catch {
      continue;
    }
  }
  return out;
}

/**
 * Keeps the `zeyn_tz` cookie equal to the browser time zone. When the server rendered the page in
 * another zone (`meta[name=zeyn-tz]`) and the user did not pick one by hand (`zeyn_tz_manual`),
 * reloads once per session so the server can use the new cookie. No date math happens here.
 */
function syncTimeZone() {
  let zone;
  try {
    zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  } catch {
    return;
  }
  if (!zone) return;
  const jar = cookies();
  if (jar.zeyn_tz !== zone) {
    document.cookie = `zeyn_tz=${encodeURIComponent(zone)}; path=${BASE || '/'}; max-age=31536000; samesite=lax`;
  }
  const served = document.querySelector('meta[name="zeyn-tz"]')?.content;
  if (!served || served === zone || 'zeyn_tz_manual' in jar) return;
  try {
    if (sessionStorage.getItem('zeyn_tz_reloaded')) return;
    sessionStorage.setItem('zeyn_tz_reloaded', '1');
  } catch {
    return;
  }
  location.reload();
}

/** Abort controller of the currently registered WebMCP tools. */
let agentTools = { controller: null, names: [], handles: [], seq: 0 };

/**
 * Registers the WebMCP tools of the top visible screen and unregisters the previous ones.
 *
 * Uses `document.modelContext.registerTool(tool, { signal })` from the WebMCP draft
 * (unregistering by aborting the signal), and falls back to the older `navigator.modelContext`
 * with `unregisterTool(name)` or a returned `{ unregister() }` handle. Does nothing when the
 * browser has no model context or `GET /_zeyn/tools` answers with an error (agent tools off).
 */
async function syncAgentTools() {
  const mc = document.modelContext ?? navigator.modelContext;
  if (!mc?.registerTool) return;
  const seq = ++agentTools.seq;
  agentTools.controller?.abort();
  for (const h of agentTools.handles) h?.unregister?.();
  for (const name of agentTools.names) mc.unregisterTool?.(name);
  agentTools = { controller: null, names: [], handles: [], seq };

  const screen = dialogs.at(-1)?.dialog.querySelector('[data-zeyn-screen]') ?? document.querySelector('.z-main [data-zeyn-screen]');
  if (!screen) return;
  const name = screen.dataset.zeynScreen;
  const params = paramsOf(screen);
  let list;
  try {
    const q = params == null ? '' : `&params=${encodeURIComponent(JSON.stringify(params))}`;
    const res = await fetch(`${BASE}/_zeyn/tools?screen=${encodeURIComponent(name)}${q}`, { credentials: 'same-origin' });
    if (!res.ok) return;
    list = (await res.json())?.tools ?? [];
  } catch {
    return;
  }
  if (seq !== agentTools.seq) return;

  const controller = new AbortController();
  agentTools.controller = controller;
  for (const tool of list) {
    const base = { screen: name, params, node: tool.node };
    const execute = async (input) => {
      if (tool.kind === 'grid') {
        const r = await post(BASE + '/_zeyn/grid', { ...base, query: input }, { quiet: true });
        return { status: r.status, ...r.data, text: r.data?.rows ? rowsText(r.data.rows) : '' };
      }
      const r = await post(BASE + '/_zeyn/form', { ...base, values: input, via_agent: true }, { quiet: true });
      if (r.ok) await runEffects(r.data?.effects);
      return { status: r.status, ...r.data };
    };
    try {
      const handle = await mc.registerTool(
        { name: tool.name, description: tool.description, inputSchema: tool.inputSchema, execute },
        { signal: controller.signal },
      );
      agentTools.names.push(tool.name);
      agentTools.handles.push(handle);
    } catch {
      continue;
    }
  }
}

/** Turns rows HTML into tab separated text lines without running any markup. */
function rowsText(html) {
  const t = document.createElement('template');
  t.innerHTML = `<table><tbody>${html}</tbody></table>`;
  return [...t.content.querySelectorAll('tr')]
    .map((tr) => [...tr.cells].map((c) => c.textContent.trim()).join('\t'))
    .join('\n');
}

document.addEventListener('click', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t) return;
  let el;
  if ((el = t.closest('button.z-sort'))) return toggleSort(el);
  if ((el = t.closest('[data-zeyn-page]'))) return loadGrid(el.closest('[data-zeyn-grid]'), el.dataset.zeynPage === 'prev' ? 'prev' : 'next');
  if ((el = t.closest('[data-zeyn-more] button'))) return loadGrid(el.closest('[data-zeyn-grid]'), 'more');
  if ((el = t.closest('[data-zeyn-export]'))) return exportGrid(el);
  if ((el = t.closest('[data-zeyn-close]'))) return el.closest('dialog')?.close();
  if ((el = t.closest('button[data-zeyn-effect], a[data-zeyn-effect]'))) {
    e.preventDefault();
    const effect = parseEffect(el);
    if (effect) runEffect(effect, el);
  }
});

document.addEventListener('focusin', (e) => {
  const tr = e.target;
  if (tr instanceof HTMLTableRowElement && tr.parentElement?.matches('[data-zeyn-rows]')) activate(tr, false);
});

document.addEventListener('dblclick', (e) => {
  const tr = e.target instanceof Element ? e.target.closest('[data-zeyn-rows] > tr[data-zeyn-effect]') : null;
  const effect = tr && parseEffect(tr);
  if (effect) runEffect(effect, tr);
});

document.addEventListener('keydown', (e) => {
  const t = e.target;
  const vg = t instanceof Element ? t.closest('[data-zeyn-grid][data-zeyn-paging="virtual"]') : null;
  if (vg && (t === vg || t.parentElement?.matches('[data-zeyn-rows]')) && virtualKey(e, vg)) return;
  if (t instanceof HTMLTableRowElement && t.parentElement?.matches('[data-zeyn-rows]') && rowKey(e, t)) return;
  shortcut(e);
});

document.addEventListener('submit', (e) => {
  const form = e.target;
  if (!(form instanceof HTMLFormElement) || !form.matches('[data-zeyn-submit]')) return;
  e.preventDefault();
  submitForm(form, e.submitter);
});

document.addEventListener('input', (e) => {
  const t = e.target;
  if (!(t instanceof Element)) return;
  if (t.matches('input[data-zeyn-filter]')) return onFilter(t);
  if (t.getAttribute('aria-invalid') === 'true') {
    t.removeAttribute('aria-invalid');
    const box = t.id && document.getElementById(`${t.id}-error`);
    if (box) box.textContent = '';
  }
});

document.addEventListener('change', (e) => {
  if (!(e.target instanceof HTMLSelectElement)) return;
  if (e.target.matches('[data-zeyn-locale]')) changeLocale(e.target);
  else if (e.target.matches('[data-zeyn-timezone]')) changeTimeZone(e.target);
});

syncTimeZone();
initGrids(document);
syncAgentTools();

document.addEventListener('zeyn:content', (e) => {
  initGrids(e.detail);
  syncAgentTools();
});
