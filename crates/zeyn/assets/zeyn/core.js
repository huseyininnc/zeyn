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
/** Most toasts shown at once (`MAX_TOASTS` on the server); the oldest goes first. */
const MAX_TOASTS = 3;
const FILTER_DELAY_MS = 300;
const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([type="hidden"]):not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
const UNREACHABLE = 'The server could not be reached.';
const V_CAP = 8_000_000;
const V_OVERSCAN = 20;
const V_CACHE = 20;
/** Most cells the parsed blocks of one virtual grid keep, fresh and stale together (8 blocks of 200 rows at 62 columns). */
const V_CACHE_CELLS = 100_000;
const V_FETCH_DELAY_MS = 40;
const VIEW_SAVE_DELAY_MS = 500;
const MIN_COLUMN = 48;
const MAX_COLUMN = 1200;
const MAC = /mac|iphone|ipad/i.test(navigator.userAgentData?.platform ?? navigator.platform ?? '');

/** @type {WeakMap<Element, object>} Per grid root state. */
const grids = new WeakMap();
/** @type {{dialog: HTMLDialogElement, opener: Element | null}[]} Open windows, top last. */
const dialogs = [];

/** The `?v=` build query this script was loaded with, for the modules it imports later. */
const VERSION = new URL(import.meta.url).search;

/**
 * The grid runtime (`/_zeyn/grid.js`) once it has loaded. Pages without a grid never load it, so
 * every call into it goes through `G?.`: before it loads there is no grid to act on.
 * @type {object | null}
 */
let G = null;
/** @type {Promise<object> | null} The grid runtime loading, shared by all callers. */
let gridLoad = null;

/**
 * Sets up every grid inside `root`, loading the grid runtime the first time a page holds a grid
 * (on start, after `nav.js` swaps content in, in a window). Safe to call more than once.
 * @returns {Promise<void>}
 */
function initGrids(root) {
  if (!G && !root.querySelector?.('[data-zeyn-grid]')) return Promise.resolve();
  gridLoad ??= import(`${BASE}/_zeyn/grid.js${VERSION}`)
    .then((m) => {
      m.install({ BASE, FILTER_DELAY_MS, MAX_COLUMN, MIN_COLUMN, UNREACHABLE, VIEW_SAVE_DELAY_MS, V_CACHE, V_CACHE_CELLS, V_CAP, V_FETCH_DELAY_MS, V_OVERSCAN, address, fill, fmt, grids, make, num, openMenu, paramsQuery, parseEffect, place, post, readJson, relativeTimes, runEffect, runEffects, str, toast });
      return (G = m);
    })
    .catch((e) => {
      gridLoad = null;
      throw e;
    });
  return gridLoad.then((m) => m.initGrids(root));
}

/** @type {Promise<string> | null} The `GET /_zeyn/csrf` request in flight, shared by all callers. */
let csrfPending = null;

/**
 * Returns the CSRF token for a `POST`, from `meta[name=zeyn-csrf]`. A page sent without a session
 * (a public screen) carries none, so that it can be cached; the first call then asks
 * `GET /_zeyn/csrf`, which sets the cookie, and keeps the token in the meta tag. When that fails
 * the token is empty and the server refuses the post with an error the user sees; nothing is
 * retried silently. The other scripts of the page reach it as `window.zeynCsrf`.
 * @returns {Promise<string>}
 */
async function csrf() {
  const meta = document.querySelector('meta[name="zeyn-csrf"]');
  if (meta?.content) return meta.content;
  csrfPending ??= fetch(`${BASE}/_zeyn/csrf`, { credentials: 'same-origin', cache: 'no-store' })
    .then((res) => (res.ok ? res.json() : null))
    .then((data) => {
      const token = typeof data?.token === 'string' ? data.token : '';
      if (meta && token) meta.content = token;
      return token;
    })
    .catch(() => '')
    .finally(() => {
      csrfPending = null;
    });
  return csrfPending;
}
window.zeynCsrf = csrf;

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
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
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

/** Returns the node path (`"0"`, `"1.0.2"`) of the closest `[data-zeyn-node]`, or `null`. */
function nodeOf(el) {
  const node = el.closest('[data-zeyn-node]');
  return node ? node.dataset.zeynNode : null;
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
 * Shows a toast for four seconds, eight for danger. The timer pauses while the pointer is over
 * the toast or it holds focus (WCAG 2.2.1). Danger toasts are alerts, announced at once; the
 * others are read politely by the region. At most `MAX_TOASTS` show; a new one removes the
 * oldest. Every toast has a dismiss button.
 * @param {string} message
 * @param {'info' | 'success' | 'warning' | 'danger' | 'error' | 'neutral'} [variant]
 * @param {{label: string, run: Function}} [action] A button on the toast.
 */
function toast(message, variant = 'info', action = null) {
  const tone = { error: 'danger', neutral: 'info', brand: 'info' }[variant] ?? variant;
  const el = document.createElement('div');
  el.className = `z-toast z-toast-${tone}${tone === 'danger' ? ' z-toast-error' : ''}`;
  if (tone === 'danger') el.setAttribute('role', 'alert');
  const text = document.createElement('span');
  text.className = 'z-toast-text';
  text.textContent = message;
  el.append(text);
  if (action) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'z-toast-action';
    button.textContent = action.label;
    button.addEventListener('click', () => {
      el.remove();
      action.run();
    });
    el.append(' ', button);
  }
  const dismiss = document.createElement('button');
  dismiss.type = 'button';
  dismiss.className = 'z-toast-close';
  dismiss.setAttribute('aria-label', document.querySelector('.z-toasts[data-text-dismiss]')?.dataset.textDismiss || 'Dismiss');
  dismiss.textContent = '\u00d7';
  dismiss.addEventListener('click', () => el.remove());
  el.append(dismiss);
  let left = tone === 'danger' ? TOAST_MS * 2 : TOAST_MS;
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
  const region = toastRegion();
  region.append(el);
  const shown = region.querySelectorAll(':scope > .z-toast');
  for (let i = 0; i < shown.length - MAX_TOASTS; i++) shown[i].remove();
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
    case 'navigate': {
      const url = `${BASE}/_zeyn/route/${encodeURIComponent(effect.screen)}${paramsQuery(effect.params)}`;
      if (!ask('zeyn:navigate', { url })) location.assign(url);
      return;
    }
    case 'go': {
      const url = new URL(effect.url ?? '/', location.href);
      if (url.origin !== location.origin) return;
      if (effect.frame) {
        history.pushState({ zeyn: true }, '', url.href);
        if (!ask('zeyn:refresh-page')) location.assign(url.href);
      } else if (!ask('zeyn:navigate', { url: url.href })) location.assign(url.href);
      return;
    }
    case 'toast':
      return toast(
        effect.message ?? '',
        effect.tone ?? 'info',
        effect.action ? { label: effect.action.label ?? '', run: () => runEffect(effect.action.effect, source) } : null,
      );
    case 'close_and_refresh':
      return closeAndRefresh();
    default:
      document.dispatchEvent(new CustomEvent('zeyn:effect', { detail: { effect, source, run: (next) => runEffect(next, source) } }));
  }
}

/**
 * Sends a cancelable request event to nav.js (`zeyn:navigate`, `zeyn:refresh`,
 * `zeyn:refresh-page`). Returns whether it was handled; when not, the caller loads the page.
 */
function ask(name, detail = {}) {
  return !document.dispatchEvent(new CustomEvent(name, { detail, cancelable: true }));
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
  await Promise.all([...under.querySelectorAll('[data-zeyn-grid]')].map((g) => G?.loadGrid(g, 'reload')));
}

