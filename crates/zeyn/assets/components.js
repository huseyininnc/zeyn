/**
 * Zeyn component library runtime.
 *
 * Browser behavior of the components in `zeyn::ui`: collapsible panels, tabs with lazy panels,
 * split panes, date pickers, lookups, line editors, confirm dialogs and trees. Interaction is
 * wired with event delegation on `document`. Components that need setup are initialized on page
 * load, on the `zeyn:content` event (nav.js and lazy panels) and when a window `<dialog>` opens.
 * Every server call goes through `/_zeyn/node` or `/_zeyn/data`, which check access on the server.
 * @module components
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
/** The `?v=` build query this script was loaded with, for the modules it imports later. */
const VERSION = new URL(import.meta.url).search;
const LOOKUP_DELAY_MS = 250;

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/**
 * Posts JSON with the CSRF header.
 * @param {string} path Endpoint below the app base, such as `/_zeyn/data`.
 * @param {object} body
 * @param {AbortSignal} [signal]
 * @returns {Promise<{ok: boolean, status: number, body: string}>} Status 0 when unreachable or aborted.
 */
async function post(path, body, signal) {
  try {
    const res = await fetch(BASE + path, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify(body),
      signal,
    });
    return { ok: res.ok, status: res.status, body: await res.text() };
  } catch {
    return { ok: false, status: 0, body: '' };
  }
}

/** Parses JSON, or returns `null`. */
function parse(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** Screen name, params and node path of an element, as the endpoints expect them. */
function address(el) {
  const screen = el.closest('[data-zeyn-screen]');
  return {
    screen: screen?.dataset.zeynScreen ?? null,
    params: parse(screen?.dataset.zeynParams ?? 'null'),
    node: el.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
  };
}

/** Calls the data source `key` of the component that holds `el`. Returns the JSON or `null`. */
async function fetchData(el, key, input, signal) {
  const r = await post('/_zeyn/data', { ...address(el), key, input }, signal);
  return r.ok ? parse(r.body) : null;
}

/** Replaces `{name}` placeholders. */
function fill(template, values) {
  return (template ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in values ? String(values[k]) : m));
}

/** Creates an element with a class and attributes. */
function make(tag, className, attrs = {}) {
  const el = document.createElement(tag);
  if (className) el.className = className;
  for (const [k, v] of Object.entries(attrs)) {
    if (k === 'text') el.textContent = v;
    else el.setAttribute(k, v);
  }
  return el;
}

/** Announces a message in the closest live region, or the page toast region. */
function announce(el, message) {
  const region = el.closest('[data-zeyn-live]')?.querySelector(':scope > .z-sr-only[aria-live]') ?? document.querySelector('.z-toasts');
  if (region) region.textContent = message;
}

/* Panels ------------------------------------------------------------------------------------- */

/** Shows or hides a panel body (disclosure pattern). */
function togglePanel(button) {
  const open = button.getAttribute('aria-expanded') !== 'true';
  button.setAttribute('aria-expanded', String(open));
  const body = document.getElementById(button.getAttribute('aria-controls'));
  if (body) body.hidden = !open;
}

/* Tabs --------------------------------------------------------------------------------------- */

/** Tabs of a tab list, in order. */
function tabsOf(tab) {
  return [...tab.closest('[role="tablist"]').querySelectorAll('[role="tab"]')];
}

/** Hash state: `#orders-tabs-1=lines&other=2` as a map. */
function hashState() {
  return new URLSearchParams(location.hash.slice(1));
}

/**
 * Opens a tab: marks it selected, shows its panel, loads a lazy panel the first time and
 * remembers the choice in the URL hash.
 */
function selectTab(tab, { focus = false, remember = true } = {}) {
  const root = tab.closest('[data-zeyn-tabs]');
  for (const t of tabsOf(tab)) {
    const on = t === tab;
    t.setAttribute('aria-selected', String(on));
    t.tabIndex = on ? 0 : -1;
    const panel = document.getElementById(t.getAttribute('aria-controls'));
    if (panel) panel.hidden = !on;
    if (on && panel?.dataset.zeynLazy) loadPanel(panel, root);
  }
  if (focus) tab.focus();
  if (remember && root) {
    const state = hashState();
    if (tab === tabsOf(tab)[0]) state.delete(root.dataset.zeynTabs);
    else state.set(root.dataset.zeynTabs, tab.dataset.zeynTab);
    const hash = state.toString();
    history.replaceState(history.state, '', hash ? `#${hash}` : location.pathname + location.search);
  }
}

/** Loads a lazy tab panel from `/_zeyn/node` and initializes what it contains. */
async function loadPanel(panel, root) {
  const path = panel.dataset.zeynLazy;
  delete panel.dataset.zeynLazy;
  panel.setAttribute('aria-busy', 'true');
  panel.replaceChildren(make('p', 'z-muted', { text: root?.dataset.zeynLoading ?? '' }));
  const a = address(panel);
  const r = await post('/_zeyn/node', { screen: a.screen, params: a.params, node: path });
  panel.removeAttribute('aria-busy');
  if (!r.ok) {
    panel.dataset.zeynLazy = path;
    const message = parse(r.body)?.message ?? root?.dataset.zeynFailed ?? '';
    panel.replaceChildren(make('p', 'z-callout z-tone-danger', { role: 'alert', text: message }));
    return;
  }
  panel.innerHTML = r.body;
  document.dispatchEvent(new CustomEvent('zeyn:content', { detail: panel }));
}

/** Arrow keys, Home and End on a tab (ARIA tabs pattern with automatic activation). */
function tabKey(e, tab) {
  const tabs = tabsOf(tab);
  const i = tabs.indexOf(tab);
  const next = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: tabs.length - 1 }[e.key];
  if (next === undefined) return false;
  e.preventDefault();
  selectTab(tabs[(next + tabs.length) % tabs.length], { focus: true });
  return true;
}

/** Opens the tabs named in the URL hash. */
function restoreTabs(root) {
  const state = hashState();
  root.querySelectorAll('[data-zeyn-tabs]').forEach((tabs) => {
    const wanted = state.get(tabs.dataset.zeynTabs);
    const tab = wanted && [...tabs.querySelectorAll('[role="tab"]')].find((t) => t.dataset.zeynTab === wanted);
    if (tab && tab.getAttribute('aria-selected') !== 'true') selectTab(tab, { remember: false });
  });
}

/* Split panes -------------------------------------------------------------------------------- */

/** Sets the share of the first pane, 10 to 90 percent. */
function setSplit(bar, percent) {
  const value = Math.round(Math.min(90, Math.max(10, percent)));
  bar.closest('[data-zeyn-split]').style.setProperty('--z-split', `${value}%`);
  bar.setAttribute('aria-valuenow', String(value));
}

/** Drags a split divider with the pointer. */
function dragSplit(e, bar) {
  const split = bar.closest('[data-zeyn-split]');
  const vertical = split.classList.contains('z-split-v');
  bar.setPointerCapture(e.pointerId);
  bar.dataset.dragging = '';
  e.preventDefault();
  const move = (ev) => {
    const box = split.getBoundingClientRect();
    const percent = vertical ? ((ev.clientY - box.top) / box.height) * 100 : ((ev.clientX - box.left) / box.width) * 100;
    setSplit(bar, percent);
  };
  const up = () => {
    delete bar.dataset.dragging;
    bar.removeEventListener('pointermove', move);
    bar.removeEventListener('pointerup', up);
    bar.removeEventListener('pointercancel', up);
  };
  bar.addEventListener('pointermove', move);
  bar.addEventListener('pointerup', up);
  bar.addEventListener('pointercancel', up);
}

/** Arrow keys move a focused divider by 5 percent, Home and End to the limits. */
function splitKey(e, bar) {
  const now = Number(bar.getAttribute('aria-valuenow')) || 40;
  const step = { ArrowLeft: -5, ArrowUp: -5, ArrowRight: 5, ArrowDown: 5 }[e.key];
  if (step !== undefined) setSplit(bar, now + step);
  else if (e.key === 'Home') setSplit(bar, 10);
  else if (e.key === 'End') setSplit(bar, 90);
  else return false;
  e.preventDefault();
  return true;
}

/* Popup core --------------------------------------------------------------------------------- */

/**
 * The one open popup: `{ panel, anchor, owner, onClose, trap }`. Every control opens its panel
 * through `openPopup`, so positioning, dismissal and focus behave the same everywhere. Panels
 * are manual popovers in the top layer, so they escape scroll containers, sticky headers and
 * window dialogs; they stay inside the viewport, open upward when there is no room below and
 * become bottom sheets on narrow screens.
 */
let popup = null;
const NARROW = window.matchMedia('(max-width: 640px)');

/**
 * Opens a panel next to `anchor`. Focus returns to `owner` when it closes with Escape.
 * @param {HTMLElement} panel
 * @param {{anchor: Element, owner?: HTMLElement, onClose?: Function, trap?: boolean}} options
 *   `trap` keeps Tab inside the panel (modal dialogs such as the calendar).
 */
function openPopup(panel, { anchor, owner, onClose, trap = true }) {
  closePopup(false);
  panel.classList.add('z-pop');
  panel.setAttribute('popover', 'manual');
  (anchor.closest('dialog') ?? document.body).append(panel);
  try {
    panel.showPopover();
  } catch {
    panel.classList.add('z-pop-fallback');
  }
  popup = { panel, anchor, owner, onClose, trap };
  placePopup();
  return panel;
}

/** Positions the open popup below its anchor, or above it when there is no room below. */
function placePopup() {
  if (!popup) return;
  const { panel, anchor } = popup;
  const sheet = NARROW.matches;
  panel.classList.toggle('z-sheet', sheet);
  if (sheet) return;
  const a = anchor.getBoundingClientRect();
  const w = panel.offsetWidth;
  const h = panel.offsetHeight;
  const room = innerHeight - a.bottom - 8;
  const up = room < h && a.top - 8 > room;
  const top = up ? Math.max(8, a.top - h - 4) : Math.max(8, Math.min(a.bottom + 4, innerHeight - h - 8));
  const rtl = getComputedStyle(anchor).direction === 'rtl';
  const left = Math.max(8, Math.min(rtl ? a.right - w : a.left, innerWidth - w - 8));
  panel.style.setProperty('--z-pop-x', `${Math.round(left)}px`);
  panel.style.setProperty('--z-pop-y', `${Math.round(top)}px`);
  panel.dataset.side = up ? 'top' : 'bottom';
}

/** Closes the open popup. Focus goes back to its owner unless the user clicked elsewhere. */
function closePopup(returnFocus = true) {
  if (!popup) return;
  const { panel, owner, onClose } = popup;
  popup = null;
  try {
    panel.hidePopover();
  } catch {
    /* not shown as a popover */
  }
  panel.remove();
  onClose?.();
  if (returnFocus) owner?.focus();
}

/** Visible, focusable elements of a panel in order. */
function focusables(panel) {
  return [...panel.querySelectorAll('button:not([disabled]), [tabindex="0"], input:not([type="hidden"]):not([disabled])')].filter(
    (el) => el.getClientRects().length > 0 && el.getAttribute('tabindex') !== '-1',
  );
}

/** Escape closes the popup; Tab stays inside a trapping popup. */
function popupKey(e) {
  if (!popup) return false;
  const inside = popup.panel.contains(e.target);
  if (e.key === 'Escape' && (inside || popup.anchor.contains(e.target))) {
    e.preventDefault();
    e.stopPropagation();
    if (inside && popup.panel.onEscape?.()) return true;
    closePopup(true);
    return true;
  }
  if (e.key === 'Tab' && inside && popup.trap) {
    const items = focusables(popup.panel);
    if (!items.length) return false;
    const i = items.indexOf(document.activeElement);
    const next = e.shiftKey ? (i <= 0 ? items.length - 1 : i - 1) : (i + 1) % items.length;
    e.preventDefault();
    items[next].focus();
    return true;
  }
  return false;
}

/* Date and time pickers ---------------------------------------------------------------------- */

/**
 * The open picker: `{ root, kind, input, inputs, bound, view, focus, from, to, time, panel }`.
 * `focus` is the ISO date the calendar shows; `time` is `{h, m, s}` or `null`.
 */
let picker = null;
/** Input events the pickers dispatch themselves; they reach the grid filters. */
const own = new WeakSet();
const CHECK_DELAY_MS = 500;

/** Page language; names of months and days follow it, not the browser's language. */
function lang() {
  return document.documentElement.lang || 'en';
}

/** Localized texts of a picker root, from `data-texts`. */
function texts(root) {
  root.zeynTexts ??= parse(root.dataset.texts ?? '{}') ?? {};
  return root.zeynTexts;
}

const pad2 = (n) => String(n).padStart(2, '0');

/** `2024-01-05` from parts. */
function iso(y, m, d) {
  return `${String(y).padStart(4, '0')}-${pad2(m)}-${pad2(d)}`;
}

/** A UTC date from an ISO date, so the browser's zone never shifts the day shown. */
function utc(isoDate) {
  const [y, m, d] = isoDate.split('-').map(Number);
  return new Date(Date.UTC(y, m - 1, d));
}

/** ISO date `days` after `isoDate`. Used to move the calendar focus, never to change a value. */
function addDays(isoDate, days) {
  const t = utc(isoDate);
  t.setUTCDate(t.getUTCDate() + days);
  return t.toISOString().slice(0, 10);
}

/** ISO date `months` after `isoDate`, on the same day or the last day of a shorter month. */
function addMonths(isoDate, months) {
  const [y, m, d] = isoDate.split('-').map(Number);
  const first = new Date(Date.UTC(y, m - 1 + months, 1));
  const last = new Date(Date.UTC(first.getUTCFullYear(), first.getUTCMonth() + 1, 0)).getUTCDate();
  return iso(first.getUTCFullYear(), first.getUTCMonth() + 1, Math.min(d, last));
}

/** The date part of a full typed value in the locale's order, as ISO; or `null`. */
function readDate(root, text) {
  const head = (text ?? '').trim().split(/[ T]/)[0];
  if (/^\d{4}-\d{2}-\d{2}$/.test(head)) return head;
  const parts = head.split(/[./-]/);
  if (parts.length !== 3 || parts.some((p) => !/^\d+$/.test(p))) return null;
  const order = root.dataset.order;
  const [d, m, y] = order === 'mdy' ? [parts[1], parts[0], parts[2]] : order === 'ymd' ? [parts[2], parts[1], parts[0]] : parts;
  if (y.length !== 4) return null;
  const out = iso(Number(y), Number(m), Number(d));
  return utc(out).toISOString().slice(0, 10) === out ? out : null;
}

/** Writes an ISO date in the locale's short form, exactly as the server formats it. */
function writeDate(root, isoDate) {
  const [y, m, d] = isoDate.split('-');
  const pad = root.dataset.pad === 'true';
  const dd = pad ? d : String(Number(d));
  const mm = pad ? m : String(Number(m));
  const sep = root.dataset.sep;
  if (root.dataset.order === 'mdy') return `${mm}${sep}${dd}${sep}${y}`;
  if (root.dataset.order === 'ymd') return isoDate;
  return `${dd}${sep}${mm}${sep}${y}`;
}

/** The time part of a typed value: everything after the date for date time fields. */
function timeText(root, text) {
  const kind = root.dataset.zeynDate;
  return kind === 'time' ? text.trim() : text.trim().split(/[ T]/).slice(1).join(' ');
}

/** Reads `14:30`, `14:30:15` or `2:30 PM` into `{h, m, s}`; or `null`. */
function readTime(text) {
  const t = /^(\d{1,2}):(\d{2})(?::(\d{2}))?\s*([AaPp][Mm])?$/.exec((text ?? '').trim());
  if (!t) return null;
  let h = Number(t[1]);
  if (t[4]) {
    if (h < 1 || h > 12) return null;
    h = (h % 12) + (/p/i.test(t[4]) ? 12 : 0);
  }
  const m = Number(t[2]);
  const s = Number(t[3] ?? 0);
  return h < 24 && m < 60 && s < 60 ? { h, m, s } : null;
}

/** Writes a time the way the server shows it: `14:30`, `2:30 PM`; seconds when used. */
function writeTime(root, { h, m, s }) {
  const secs = root.dataset.seconds === 'true' || s ? `:${pad2(s)}` : '';
  if (root.dataset.h12 === 'true') return `${h % 12 || 12}:${pad2(m)}${secs} ${h < 12 ? 'AM' : 'PM'}`;
  return `${pad2(h)}:${pad2(m)}${secs}`;
}

/** Today in the user's time zone, as rendered by the server. */
function todayOf(root) {
  if (root.dataset.today) return root.dataset.today;
  const n = new Date();
  return iso(n.getFullYear(), n.getMonth() + 1, n.getDate());
}

/** Whether a day cannot be chosen: outside min and max, a weekend when weekends are off, or listed. */
function blocked(root, isoDate) {
  const d = root.dataset;
  if ((d.min && isoDate < d.min) || (d.max && isoDate > d.max)) return true;
  if (d.weekends === 'off' && [0, 6].includes(utc(isoDate).getUTCDay())) return true;
  return !!d.disabled && d.disabled.split(',').includes(isoDate);
}

/** ISO week number of a day. */
function isoWeek(isoDate) {
  const t = utc(isoDate);
  t.setUTCDate(t.getUTCDate() + 3 - ((t.getUTCDay() + 6) % 7));
  const jan4 = new Date(Date.UTC(t.getUTCFullYear(), 0, 4));
  return 1 + Math.round(((t - jan4) / 86400000 - 3 + ((jan4.getUTCDay() + 6) % 7)) / 7);
}

/** The label of a date input, without the required star. */
function labelOf(input) {
  const label = input.id && document.querySelector(`label[for="${CSS.escape(input.id)}"]`);
  return label ? label.textContent.replace('*', '').trim() : input.getAttribute('aria-label') ?? '';
}

/** Sets an input's value as the user would, so the runtime and the grid filters notice. */
function setValue(input, text) {
  input.value = text;
  const ev = new Event('input', { bubbles: true });
  own.add(ev);
  input.dispatchEvent(ev);
  input.dispatchEvent(new Event('change', { bubbles: true }));
}

/**
 * Asks the server to read what was typed, then shows its canonical text, or its message under
 * the field. The server decides every value; the browser only shows the answer.
 */
async function check(input) {
  const root = input.closest('[data-zeyn-date]');
  if (!root) return;
  const text = input.value;
  if (input.dataset.checked === text) return;
  const kind = root.dataset.zeynDate === 'range' ? 'date' : root.dataset.zeynDate;
  const r = await post('/_zeyn/parse', { kind, text, label: labelOf(input) });
  const data = r.ok ? parse(r.body) : null;
  if (!data || input.value !== text) return;
  const box = input.id && document.getElementById(`${input.id}-error`);
  if (data.ok) {
    if (data.text !== text) input.value = data.text;
    input.dataset.checked = input.value;
    const changed = input.dataset.iso !== data.iso;
    input.dataset.iso = data.iso;
    if (input.getAttribute('aria-invalid') === 'true' && box?.dataset.zeynDateError !== undefined) input.removeAttribute('aria-invalid');
    if (box?.dataset.zeynDateError !== undefined) {
      box.textContent = '';
      delete box.dataset.zeynDateError;
    }
    showZone(root, data.utc);
    if (changed && input.dataset.zeynFilterDate) {
      const ev = new Event('input', { bubbles: true });
      own.add(ev);
      input.dispatchEvent(ev);
    }
  } else {
    input.setAttribute('aria-invalid', 'true');
    if (box) {
      box.textContent = data.message;
      box.dataset.zeynDateError = '';
    }
  }
}

/** Updates the zone chip of a moment field with the UTC moment from the server. */
function showZone(root, moment) {
  const chip = root.querySelector('[data-zeyn-zone]');
  if (!chip) return;
  const text = moment ? fill(texts(root).utc, { moment }) : '';
  chip.title = text;
  const sr = chip.querySelector('[data-zeyn-utc]');
  if (sr) sr.textContent = text ? `, ${text}` : '';
}

/**
 * Formats while typing: separators are added after the day, month and hour, and digits that
 * cannot start a two digit part are split (`45` becomes `4.5`, `93` becomes `9:3`). Only plain
 * digits at the end are touched, so words such as `today` or `+3d` stay as typed.
 */
function mask(input, e) {
  if (e.inputType !== 'insertText' || !/^\d$/.test(e.data ?? '') || input.selectionStart !== input.value.length) return;
  const root = input.closest('[data-zeyn-date]');
  const kind = root.dataset.zeynDate;
  const v = input.value;
  const clock = (t, prefix) => {
    const max = root.dataset.h12 === 'true' ? 12 : 23;
    if (/^\d{2}$/.test(t)) input.value = Number(t) > max ? `${prefix}${t[0]}:${t[1]}` : `${prefix}${t}:`;
    else if (/^\d{1,2}:\d{2}$/.test(t) && root.dataset.seconds === 'true') input.value = `${prefix}${t}:`;
  };
  if (kind === 'time') return clock(v, '');
  if (v.includes(' ')) {
    const at = v.indexOf(' ');
    return clock(v.slice(at + 1), v.slice(0, at + 1));
  }
  if (!/^[\d./-]+$/.test(v)) return;
  const sep = root.dataset.sep;
  const order = root.dataset.order;
  const parts = v.split(/[./-]/);
  const last = parts.at(-1);
  const roles = order === 'mdy' ? ['m', 'd', 'y'] : order === 'ymd' ? ['y', 'm', 'd'] : ['d', 'm', 'y'];
  const role = roles[parts.length - 1];
  const head = v.slice(0, v.length - last.length);
  if (role === 'y') {
    if (last.length === 4 && parts.length === 3 && (kind === 'datetime' || kind === 'local')) input.value = `${v} `;
    else if (last.length === 4 && parts.length === 1) input.value = `${v}${sep}`;
    return;
  }
  if (last.length !== 2 || parts.length > 2) return;
  const limit = role === 'd' ? 31 : 12;
  input.value = Number(last) > limit ? `${head}${last[0]}${sep}${last[1]}` : `${v}${sep}`;
}

/** Arrow Up and Down change the hour, minute or second under the caret of a time input. */
function stepTime(input, e) {
  const root = input.closest('[data-zeyn-date]');
  if (root?.dataset.zeynDate !== 'time' || !['ArrowUp', 'ArrowDown'].includes(e.key) || e.altKey) return false;
  const t = readTime(input.value);
  if (!t) return false;
  e.preventDefault();
  const caret = input.selectionStart ?? 0;
  const colons = [...input.value.matchAll(/:/g)].map((x) => x.index);
  const seg = caret <= colons[0] ? 'h' : colons[1] !== undefined && caret > colons[1] ? 's' : 'm';
  const dir = e.key === 'ArrowUp' ? 1 : -1;
  const step = Number(root.dataset.step) || 1;
  if (seg === 'h') t.h = (t.h + dir + 24) % 24;
  else if (seg === 'm') t.m = (Math.floor(t.m / step) * step + dir * step + 60) % 60;
  else t.s = (t.s + dir + 60) % 60;
  setValue(input, writeTime(root, t));
  input.setSelectionRange(caret, caret);
  return true;
}

/** Opens the picker of a root for one of its inputs. */
function openPicker(root, input) {
  const kind = root.dataset.zeynDate;
  const inputs = [...root.querySelectorAll('[data-zeyn-date-input]')];
  input ??= inputs.includes(document.activeElement) ? document.activeElement : inputs[0];
  const t = texts(root);
  const button = root.querySelector('[data-zeyn-date-open]');
  const range = kind === 'range';
  const from = range ? readDate(root, inputs[0].value) : null;
  const to = range ? readDate(root, inputs[1].value) : null;
  let focus = (range ? (input === inputs[1] ? to ?? from : from) : readDate(root, input.value)) ?? todayOf(root);
  if (root.dataset.min && focus < root.dataset.min) focus = root.dataset.min;
  if (root.dataset.max && focus > root.dataset.max) focus = root.dataset.max;
  const label = kind === 'time' ? t.dialog_time : range ? t.dialog_range : t.dialog;
  const panel = make('div', `z-picker z-picker-${kind}`, { role: 'dialog', 'aria-modal': 'true', 'aria-label': label });
  picker = {
    root,
    kind,
    input,
    inputs,
    bound: range && input === inputs[1] && from ? 1 : 0,
    view: 'days',
    focus,
    from,
    to: range && input === inputs[0] && !to ? null : to,
    time: kind === 'date' || range ? null : readTime(timeText(root, input.value)),
    panel,
  };
  if (range) panel.append(presetList(root));
  if (kind !== 'time') panel.append(make('div', 'z-cal'));
  if (kind !== 'date' && !range) panel.append(clockColumns(root));
  panel.append(pickerFoot(root));
  panel.onEscape = () => {
    if (picker?.view === 'days') return false;
    picker.view = 'days';
    drawCalendar(true);
    return true;
  };
  button?.setAttribute('aria-expanded', 'true');
  openPopup(panel, {
    anchor: root,
    owner: button ?? input,
    onClose: () => {
      button?.setAttribute('aria-expanded', 'false');
      const closing = picker;
      picker = null;
      closing?.inputs.forEach((i) => check(i));
    },
  });
  if (kind === 'time') {
    drawClock(true);
    return;
  }
  drawCalendar(true);
  if (kind !== 'date' && !range) drawClock(false);
  placePopup();
}

/** Quick ranges of a range picker, from `data-presets` (worked out on the server). */
function presetList(root) {
  const t = texts(root);
  const box = make('div', 'z-presets', { role: 'group', 'aria-label': t.presets });
  for (const p of parse(root.dataset.presets ?? '[]') ?? []) {
    box.append(make('button', '', { type: 'button', 'data-preset-from': p.from, 'data-preset-to': p.to, text: p.label }));
  }
  return box;
}

/** Footer buttons: today or now, clear, and done for pickers that stay open while choosing. */
function pickerFoot(root) {
  const t = texts(root);
  const kind = root.dataset.zeynDate;
  const foot = make('div', 'z-picker-foot');
  if (kind === 'datetime') {
    const chip = root.querySelector('[data-zeyn-zone]');
    if (chip) foot.append(make('span', 'z-date-zone', { text: chip.textContent.replace(chip.querySelector('[data-zeyn-utc]')?.textContent ?? '', '') }));
  }
  foot.append(make('span', 'z-grow'));
  if (kind === 'time') foot.append(make('button', 'z-link', { type: 'button', 'data-pick': 'now', text: t.now }));
  else if (kind !== 'range') foot.append(make('button', 'z-link', { type: 'button', 'data-pick': 'today', text: t.today }));
  foot.append(make('button', 'z-link', { type: 'button', 'data-pick': 'clear', text: t.clear }));
  if (kind !== 'date' && kind !== 'time') foot.append(make('button', 'z-button z-primary', { type: 'button', 'data-pick': 'done', text: t.done }));
  return foot;
}

/** Draws the calendar view (days, months or years) and moves focus into it when asked. */
function drawCalendar(moveFocus) {
  const { root, panel, view } = picker;
  const cal = panel.querySelector('.z-cal');
  const t = texts(root);
  const locale = lang();
  const focus = picker.focus;
  const [y, m] = focus.split('-').map(Number);
  const head = make('div', 'z-cal-head');
  const titleId = `z-cal-title-${root.querySelector('input')?.id ?? 'x'}`;
  const nav = (step, label, text) => make('button', 'z-cal-nav', { type: 'button', 'data-step': step, 'aria-label': label, text });
  const monthName = new Intl.DateTimeFormat(locale, { month: 'long', timeZone: 'UTC' }).format(utc(focus));
  const title = make('h2', 'z-sr-only', { id: titleId, 'aria-live': 'polite', text: new Intl.DateTimeFormat(locale, { month: 'long', year: 'numeric', timeZone: 'UTC' }).format(utc(focus)) });
  const monthButton = make('button', 'z-cal-title', { type: 'button', 'data-view': 'months', 'aria-label': `${t.month}, ${monthName}`, 'aria-pressed': String(view === 'months'), text: monthName });
  const yearButton = make('button', 'z-cal-title', { type: 'button', 'data-view': 'years', 'aria-label': `${t.year}, ${y}`, 'aria-pressed': String(view === 'years'), text: String(y) });
  head.append(nav(view === 'years' ? -240 : view === 'months' ? -12 : -1, view === 'days' ? t.prev_month : t.prev_year, '‹'), title, monthButton, yearButton, nav(view === 'years' ? 240 : view === 'months' ? 12 : 1, view === 'days' ? t.next_month : t.next_year, '›'));
  let body;
  if (view === 'months') body = monthGrid(root, focus, titleId);
  else if (view === 'years') body = yearGrid(root, focus);
  else body = dayGrid(root, focus, titleId);
  cal.replaceChildren(head, body);
  if (moveFocus) cal.querySelector('[tabindex="0"][data-date], [tabindex="0"][data-month], [tabindex="0"][data-year]')?.focus();
}

/** The month grid of days (ARIA date grid). */
function dayGrid(root, focus, titleId) {
  const t = texts(root);
  const locale = lang();
  const [y, m] = focus.split('-').map(Number);
  const first = Number(root.dataset.firstDay) % 7;
  const weeks = root.dataset.weeks === 'true';
  const now = todayOf(root);
  const p = picker;
  const selected = p.kind === 'range' ? [p.from, p.to].filter(Boolean) : [readDate(root, p.input.value)].filter(Boolean);
  const long = new Intl.DateTimeFormat(locale, { dateStyle: 'full', timeZone: 'UTC' });
  const table = make('table', 'z-cal-grid', { role: 'grid', 'aria-labelledby': titleId });
  const headRow = make('tr');
  if (weeks) headRow.append(make('th', 'z-cal-week', { scope: 'col', abbr: t.week_long, text: t.week }));
  for (let i = 0; i < 7; i++) {
    const day = new Date(Date.UTC(2024, 0, 7 + first + i));
    headRow.append(
      make('th', '', {
        scope: 'col',
        abbr: new Intl.DateTimeFormat(locale, { weekday: 'long', timeZone: 'UTC' }).format(day),
        text: new Intl.DateTimeFormat(locale, { weekday: 'short', timeZone: 'UTC' }).format(day),
      }),
    );
  }
  const thead = make('thead');
  thead.append(headRow);
  const tbody = make('tbody');
  const days = new Date(Date.UTC(y, m, 0)).getUTCDate();
  const lead = (new Date(Date.UTC(y, m - 1, 1)).getUTCDay() - first + 7) % 7;
  let row = null;
  for (let cell = 0; cell < lead + days; cell++) {
    if (cell % 7 === 0) {
      row = make('tr');
      tbody.append(row);
      if (weeks) {
        const dayInRow = iso(y, m, Math.max(1, cell - lead + 1));
        row.append(make('th', 'z-cal-week', { scope: 'row', text: String(isoWeek(dayInRow)) }));
      }
    }
    const td = make('td', '', { role: 'gridcell' });
    if (cell >= lead) {
      const d = cell - lead + 1;
      const date = iso(y, m, d);
      const b = make('button', 'z-cal-day', { type: 'button', tabindex: date === focus ? '0' : '-1', 'data-date': date, 'aria-label': long.format(utc(date)), text: String(d) });
      if (selected.includes(date)) b.setAttribute('aria-pressed', 'true');
      if (date === now) b.setAttribute('aria-current', 'date');
      if (blocked(root, date)) b.setAttribute('aria-disabled', 'true');
      if (p.kind === 'range' && p.from && p.to && date > p.from && date < p.to) b.classList.add('z-in-range');
      if (p.kind === 'range' && date === p.from) b.classList.add('z-range-start');
      if (p.kind === 'range' && date === p.to) b.classList.add('z-range-end');
      if (selected.includes(date)) td.setAttribute('aria-selected', 'true');
      td.append(b);
    }
    row.append(td);
  }
  while (row && row.children.length < 7 + (weeks ? 1 : 0)) row.append(make('td', '', { role: 'gridcell' }));
  table.append(thead, tbody);
  return table;
}

/** Twelve months to jump to, as a grid of three columns. */
function monthGrid(root, focus, titleId) {
  const t = texts(root);
  const [, m] = focus.split('-').map(Number);
  const year = focus.slice(0, 4);
  const grid = make('div', 'z-cal-pick', { role: 'grid', 'aria-label': t.months });
  const short = new Intl.DateTimeFormat(lang(), { month: 'short', timeZone: 'UTC' });
  const long = new Intl.DateTimeFormat(lang(), { month: 'long', year: 'numeric', timeZone: 'UTC' });
  for (let r = 0; r < 4; r++) {
    const row = make('div', 'z-cal-pick-row', { role: 'row' });
    for (let c = 0; c < 3; c++) {
      const month = r * 3 + c + 1;
      const cell = make('div', '', { role: 'gridcell' });
      const day = utc(iso(Number(year), month, 1));
      const b = make('button', '', { type: 'button', tabindex: month === m ? '0' : '-1', 'data-month': String(month), 'aria-label': long.format(day), 'aria-pressed': String(month === m), text: short.format(day) });
      const ym = `${year}-${pad2(month)}`;
      if ((root.dataset.min && ym < root.dataset.min.slice(0, 7)) || (root.dataset.max && ym > root.dataset.max.slice(0, 7))) b.setAttribute('aria-disabled', 'true');
      cell.append(b);
      row.append(cell);
    }
    grid.append(row);
  }
  grid.setAttribute('aria-describedby', titleId);
  return grid;
}

/** Twenty years around the focused one to jump to, as a grid of four columns. */
function yearGrid(root, focus) {
  const t = texts(root);
  const y = Number(focus.slice(0, 4));
  const start = y - (y % 20);
  const grid = make('div', 'z-cal-pick z-cal-years', { role: 'grid', 'aria-label': t.years });
  for (let r = 0; r < 5; r++) {
    const row = make('div', 'z-cal-pick-row', { role: 'row' });
    for (let c = 0; c < 4; c++) {
      const year = start + r * 4 + c;
      const cell = make('div', '', { role: 'gridcell' });
      const b = make('button', '', { type: 'button', tabindex: year === y ? '0' : '-1', 'data-year': String(year), 'aria-pressed': String(year === y), text: String(year) });
      if ((root.dataset.min && year < Number(root.dataset.min.slice(0, 4))) || (root.dataset.max && year > Number(root.dataset.max.slice(0, 4)))) b.setAttribute('aria-disabled', 'true');
      cell.append(b);
      row.append(cell);
    }
    grid.append(row);
  }
  return grid;
}

/** Hour, minute, second and AM or PM columns (one listbox each) for time and date time pickers. */
function clockColumns(root) {
  const t = texts(root);
  const box = make('div', 'z-clock');
  const h12 = root.dataset.h12 === 'true';
  const step = Number(root.dataset.step) || 1;
  const column = (unit, label, values, text) => {
    const list = make('div', 'z-clock-col', { role: 'listbox', 'aria-label': label, tabindex: '-1', 'data-unit': unit });
    for (const v of values) {
      list.append(make('div', 'z-clock-opt', { role: 'option', id: `z-clock-${unit}-${v}-${++uidCounter}`, 'data-value': String(v), 'aria-selected': 'false', text: text(v) }));
    }
    return list;
  };
  const range = (n, by = 1) => Array.from({ length: Math.ceil(n / by) }, (_, i) => i * by);
  box.append(column('h', t.hours, h12 ? [12, ...range(12).slice(1)] : range(24), (v) => (h12 ? String(v) : pad2(v))));
  box.append(column('m', t.minutes, range(60, step), pad2));
  if (root.dataset.seconds === 'true') box.append(column('s', t.seconds, range(60), pad2));
  if (h12) box.append(column('p', t.period, ['AM', 'PM'], (v) => v));
  return box;
}

let uidCounter = 0;

/**
 * Scrolls only `list` so `option` shows: centered when `center`, otherwise just enough. Unlike
 * `scrollIntoView` it never moves the page or the popup.
 */
function reveal(list, option, center = false) {
  const l = list.getBoundingClientRect();
  const o = option.getBoundingClientRect();
  if (center) list.scrollTop += o.top - l.top - (list.clientHeight - o.height) / 2;
  else if (o.top < l.top) list.scrollTop -= l.top - o.top;
  else if (o.bottom > l.bottom) list.scrollTop += o.bottom - l.bottom;
}

/**
 * Marks the chosen hour, minute and second, keeps them in view and sets the roving focus.
 * `center` puts each choice in the middle of its column, as when the picker opens.
 */
function drawClock(moveFocus, center = true) {
  const { root, panel } = picker;
  const time = picker.time;
  const h12 = root.dataset.h12 === 'true';
  const chosen = time && {
    h: h12 ? time.h % 12 || 12 : time.h,
    m: time.m,
    s: time.s,
    p: time.h < 12 ? 'AM' : 'PM',
  };
  const lists = [...panel.querySelectorAll('.z-clock-col')];
  lists.forEach((list, i) => {
    let active = null;
    for (const o of list.children) {
      const on = !!chosen && String(chosen[list.dataset.unit]) === o.dataset.value;
      o.setAttribute('aria-selected', String(on));
      if (on) active = o;
    }
    active ??= list.children[0];
    list.setAttribute('aria-activedescendant', active.id);
    list.tabIndex = i === 0 ? 0 : -1;
    reveal(list, active, center);
  });
  if (moveFocus) lists[0]?.focus();
}

/** Applies a value chosen in a clock column and writes the input. */
function chooseTime(list, option) {
  const root = picker.root;
  const h12 = root.dataset.h12 === 'true';
  const t = picker.time ?? { h: h12 ? 12 : 0, m: 0, s: 0 };
  const v = option.dataset.value;
  const pm = t.h >= 12;
  if (list.dataset.unit === 'h') t.h = h12 ? (Number(v) % 12) + (pm ? 12 : 0) : Number(v);
  if (list.dataset.unit === 'm') t.m = Number(v);
  if (list.dataset.unit === 's') t.s = Number(v);
  if (list.dataset.unit === 'p') t.h = (t.h % 12) + (v === 'PM' ? 12 : 0);
  picker.time = t;
  writePicked();
  drawClock(false, false);
  list.focus({ preventScroll: true });
}

/** Writes what the picker holds into its inputs, in the locale's form. */
function writePicked() {
  const p = picker;
  const root = p.root;
  if (p.kind === 'range') {
    setValue(p.inputs[0], p.from ? writeDate(root, p.from) : '');
    setValue(p.inputs[1], p.to ? writeDate(root, p.to) : '');
    return;
  }
  const date = p.kind === 'time' ? null : readDate(root, p.input.value) ?? p.chosen;
  const time = p.time ? writeTime(root, p.time) : '';
  if (p.kind === 'time') setValue(p.input, time);
  else if (p.kind === 'date') setValue(p.input, date ? writeDate(root, date) : '');
  else setValue(p.input, date ? `${writeDate(root, date)} ${time || writeTime(root, { h: 0, m: 0, s: 0 })}` : '');
}

/** Picks a day: sets the date, or a range bound, and closes when the choice is complete. */
function pickDay(date) {
  const p = picker;
  if (blocked(p.root, date)) return;
  p.focus = date;
  if (p.kind === 'range') {
    if (p.bound === 0 || !p.from || date < p.from) {
      p.from = date;
      p.to = null;
      p.bound = 1;
      writePicked();
      drawCalendar(true);
      return;
    }
    p.to = date;
    writePicked();
    closePopup(true);
    return;
  }
  p.chosen = date;
  if (p.kind === 'date') {
    setValue(p.input, writeDate(p.root, date));
    closePopup(false);
    p.input.focus();
    return;
  }
  p.input.value = '';
  writePicked();
  drawCalendar(true);
}

/** Keyboard in the calendar and the clock (ARIA date picker dialog and listbox patterns). */
function pickerKey(e) {
  if (!picker || !picker.panel.contains(e.target)) return false;
  const t = e.target;
  const list = t.closest('.z-clock-col');
  if (list) return clockKey(e, list);
  const move = (key, by) => {
    const map = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -by, ArrowDown: by };
    return map[key];
  };
  if (t.dataset.month || t.dataset.year) {
    const isMonth = !!t.dataset.month;
    const by = isMonth ? 3 : 4;
    let delta = move(e.key, by);
    if (e.key === 'PageUp') delta = isMonth ? -12 : -20;
    if (e.key === 'PageDown') delta = isMonth ? 12 : 20;
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      if (t.getAttribute('aria-disabled') === 'true') return true;
      if (isMonth) picker.focus = addMonths(picker.focus, Number(t.dataset.month) - Number(picker.focus.slice(5, 7)));
      else picker.focus = addMonths(picker.focus, (Number(t.dataset.year) - Number(picker.focus.slice(0, 4))) * 12);
      picker.view = isMonth ? 'days' : 'months';
      drawCalendar(true);
      return true;
    }
    if (delta === undefined) return false;
    e.preventDefault();
    picker.focus = addMonths(picker.focus, isMonth ? delta : delta * 12);
    drawCalendar(true);
    return true;
  }
  const day = t.dataset.date;
  if (!day) return false;
  const first = Number(picker.root.dataset.firstDay) % 7;
  const pos = (utc(day).getUTCDay() - first + 7) % 7;
  const moves = {
    ArrowLeft: () => addDays(day, -1),
    ArrowRight: () => addDays(day, 1),
    ArrowUp: () => addDays(day, -7),
    ArrowDown: () => addDays(day, 7),
    Home: () => addDays(day, -pos),
    End: () => addDays(day, 6 - pos),
    PageUp: () => addMonths(day, e.shiftKey ? -12 : -1),
    PageDown: () => addMonths(day, e.shiftKey ? 12 : 1),
  };
  if (!(e.key in moves)) return false;
  e.preventDefault();
  picker.focus = moves[e.key]();
  drawCalendar(true);
  return true;
}

/** Up, Down, Home and End choose in a clock column; Left and Right move between columns. */
function clockKey(e, list) {
  const options = [...list.children];
  const current = document.getElementById(list.getAttribute('aria-activedescendant'));
  let i = options.indexOf(current);
  const lists = [...picker.panel.querySelectorAll('.z-clock-col')];
  const li = lists.indexOf(list);
  switch (e.key) {
    case 'ArrowUp':
      i = (i - 1 + options.length) % options.length;
      break;
    case 'ArrowDown':
      i = (i + 1) % options.length;
      break;
    case 'Home':
      i = 0;
      break;
    case 'End':
      i = options.length - 1;
      break;
    case 'ArrowLeft':
    case 'ArrowRight': {
      const next = lists[li + (e.key === 'ArrowLeft' ? -1 : 1)];
      if (!next) return false;
      e.preventDefault();
      lists.forEach((l) => (l.tabIndex = l === next ? 0 : -1));
      next.focus();
      return true;
    }
    case 'Enter':
      e.preventDefault();
      if (picker.kind === 'time') closePopup(true);
      return true;
    default:
      return false;
  }
  e.preventDefault();
  chooseTime(list, options[i]);
  return true;
}

/** Clicks on picker buttons, days, months, years, clock options, presets and the footer. */
function pickerClick(e, t) {
  const open = t.closest('[data-zeyn-date-open]');
  if (open) {
    const root = open.closest('[data-zeyn-date]');
    if (picker?.root === root) closePopup(true);
    else openPicker(root);
    return true;
  }
  if (!picker || !picker.panel.contains(t)) return false;
  const p = picker;
  const option = t.closest('.z-clock-opt');
  if (option) {
    chooseTime(option.parentElement, option);
    return true;
  }
  const b = t.closest('button');
  if (!b) return true;
  if (b.dataset.date) pickDay(b.dataset.date);
  else if (b.dataset.step) {
    p.focus = addMonths(p.focus, Number(b.dataset.step));
    drawCalendar(false);
  } else if (b.dataset.view) {
    p.view = p.view === b.dataset.view ? 'days' : b.dataset.view;
    drawCalendar(true);
  } else if (b.dataset.month || b.dataset.year) {
    pickerKey({ target: b, key: 'Enter', preventDefault() {} });
  } else if (b.dataset.presetFrom) {
    p.from = b.dataset.presetFrom;
    p.to = b.dataset.presetTo;
    writePicked();
    closePopup(true);
  } else if (b.dataset.pick === 'today') {
    if (p.kind === 'date') pickDay(todayOf(p.root));
    else {
      p.focus = todayOf(p.root);
      pickDay(p.focus);
    }
  } else if (b.dataset.pick === 'now') {
    setValue(p.input, 'now');
    p.input.dataset.checked = '';
    closePopup(true);
  } else if (b.dataset.pick === 'clear') {
    p.inputs.forEach((i) => (i === p.input || p.kind === 'range') && setValue(i, ''));
    closePopup(true);
  } else if (b.dataset.pick === 'done') closePopup(true);
  return true;
}

/** Day buttons preview a range while the second bound is being chosen. */
function previewRange(e) {
  if (picker?.kind !== 'range' || !picker.from || picker.to) return;
  const day = e.target instanceof Element ? e.target.closest('[data-date]')?.dataset.date : null;
  if (!day) return;
  for (const b of picker.panel.querySelectorAll('[data-date]')) {
    b.classList.toggle('z-in-range', day > picker.from && b.dataset.date > picker.from && b.dataset.date < day);
    b.classList.toggle('z-range-end', day > picker.from && b.dataset.date === day);
  }
}

/** Debounced server checks while typing in grid filters, so the grid filters by real dates. */
const checkTimers = new WeakMap();

function scheduleCheck(input) {
  clearTimeout(checkTimers.get(input));
  checkTimers.set(input, setTimeout(() => check(input), CHECK_DELAY_MS));
}

/** Locale settings and texts for pickers built in the browser (grid filters), from the server. */
let filterConfig = null;

/**
 * Turns the date and time filter inputs of a grid column into Zeyn pickers: masks, server checks
 * and, for dates, a range picker with quick ranges.
 */
async function upgradeFilter(input) {
  const pair = input.closest('.z-filter-range');
  if (!pair || pair.dataset.zeynDate) return;
  pair.dataset.zeynDate = input.dataset.zeynFilterDate === 'time' ? 'time' : 'range';
  filterConfig ??= post('/_zeyn/parse', { kind: 'presets' }).then((r) => (r.ok ? parse(r.body) : null));
  const config = await filterConfig;
  if (!config) return;
  for (const [k, v] of Object.entries(config.data)) pair.dataset[k] = v;
  pair.dataset.texts = JSON.stringify(config.texts);
  pair.dataset.presets = JSON.stringify(config.presets);
  pair.querySelectorAll('input').forEach((i) => {
    i.dataset.zeynDateInput = '';
    i.dataset.zeynFilterDate ||= input.dataset.zeynFilterDate;
  });
  if (pair.dataset.zeynDate === 'range' && !pair.querySelector('[data-zeyn-date-open]')) {
    const b = make('button', 'z-date-open z-filter-pick', { type: 'button', 'data-zeyn-date-open': '', 'aria-haspopup': 'dialog', 'aria-expanded': 'false', 'aria-label': config.texts.dialog_range });
    b.append(make('span', 'z-date-icon', { 'aria-hidden': 'true' }));
    pair.append(b);
  }
}

/* Listbox core ------------------------------------------------------------------------------- */

/**
 * The open listbox popup: `{ combo, list, items, active, multiple, pick, more }`. Selects and
 * lookups share it: focus stays on the combobox, `aria-activedescendant` points at the active
 * option, and the popup core places and closes the panel.
 */
let lb = null;
let listUid = 0;

/** Lower case for matching in the page language, with Turkish dotted and dotless i as plain i. */
function fold(s) {
  return (s ?? '').toLocaleLowerCase(lang()).normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ı/g, 'i');
}

/**
 * Opens a listbox under a combobox.
 * @param {HTMLElement} combo The element with `role="combobox"`.
 * @param {{label: string, multiple?: boolean, pick: Function, more?: Function, anchor?: Element}} cfg
 */
function openListbox(combo, cfg) {
  const id = combo.getAttribute('aria-controls') || `z-listbox-${++listUid}`;
  const list = make('div', 'z-listbox', { role: 'listbox', id, 'aria-label': cfg.label });
  if (cfg.multiple) list.setAttribute('aria-multiselectable', 'true');
  const panel = make('div', 'z-list-pop');
  panel.append(list);
  combo.setAttribute('aria-controls', id);
  combo.setAttribute('aria-expanded', 'true');
  lb = { combo, list, panel, items: [], active: -1, multiple: !!cfg.multiple, pick: cfg.pick, more: cfg.more };
  openPopup(panel, {
    anchor: cfg.anchor ?? combo,
    owner: combo,
    trap: false,
    onClose: () => {
      combo.setAttribute('aria-expanded', 'false');
      combo.removeAttribute('aria-activedescendant');
      lb = null;
    },
  });
  const width = (cfg.anchor ?? combo).getBoundingClientRect().width;
  panel.style.setProperty('--z-pop-min', `${Math.round(width)}px`);
  return lb;
}

/**
 * Fills the open listbox. Items are `{value, label, hint?, group?, selected?, more?, note?}`;
 * a `note` item is a plain message (no results, loading).
 */
function fillListbox(items, activeValue) {
  if (!lb) return;
  const { list } = lb;
  list.replaceChildren();
  lb.items = [];
  let group = null;
  let groupEl = null;
  items.forEach((item) => {
    if (item.note) {
      list.append(make('div', 'z-list-note', { role: 'presentation', text: item.note }));
      return;
    }
    if (item.group && item.group !== group) {
      group = item.group;
      const gid = `${list.id}-g${lb.items.length}`;
      groupEl = make('div', 'z-list-group', { role: 'group', 'aria-labelledby': gid });
      groupEl.append(make('div', 'z-list-group-label', { id: gid, role: 'presentation', text: group }));
      list.append(groupEl);
    }
    const el = make('div', `z-option${item.more ? ' z-option-more' : ''}`, { role: 'option', id: `${list.id}-o${lb.items.length}`, 'aria-selected': String(!!item.selected) });
    if (lb.multiple && !item.more) el.append(make('span', 'z-option-check', { 'aria-hidden': 'true' }));
    const text = make('span', 'z-option-text');
    if (item.cells?.length && lb.columns?.length) {
      text.className = 'z-option-cells';
      item.cells.forEach((c) => text.append(make('span', '', { text: c })));
      el.setAttribute('aria-label', item.cells.map((c, i) => (lb.columns[i] ? `${lb.columns[i]}: ${c}` : c)).join(', '));
    } else {
      text.append(make('span', '', { text: item.label }));
      if (item.hint) text.append(make('span', 'z-option-hint', { text: item.hint }));
    }
    el.append(text);
    el.dataset.index = String(lb.items.length);
    (item.group ? groupEl : list).append(el);
    lb.items.push({ ...item, el });
  });
  const start = lb.items.findIndex((i) => i.value === activeValue);
  setActive(start >= 0 ? start : lb.items.length ? 0 : -1);
  placePopup();
}

/** Moves the active option and keeps it in view. */
function setActive(i) {
  if (!lb) return;
  lb.items.forEach((it, n) => it.el.classList.toggle('z-active', n === i));
  lb.active = i;
  const item = lb.items[i];
  if (item) {
    lb.combo.setAttribute('aria-activedescendant', item.el.id);
    reveal(lb.panel, item.el);
  } else lb.combo.removeAttribute('aria-activedescendant');
}

/** Picks the active option, or loads more when it is the "show more" row. */
function pickActive() {
  const item = lb?.items[lb.active];
  if (!item) return false;
  if (item.more) lb.more?.();
  else lb.pick(item);
  return true;
}

/** Arrow keys, Home, End, Page Up and Page Down in an open listbox. */
function moveActive(e) {
  if (!lb) return false;
  const n = lb.items.length;
  if (!n) return false;
  const to = { ArrowDown: lb.active + 1, ArrowUp: lb.active - 1, Home: 0, End: n - 1, PageDown: lb.active + 10, PageUp: lb.active - 10 }[e.key];
  if (to === undefined) return false;
  e.preventDefault();
  setActive(Math.max(0, Math.min(n - 1, to)));
  return true;
}

/** Option clicks pick; pressing on the list keeps focus on the combobox. */
function listboxPointer(e) {
  if (!lb || !lb.panel.contains(e.target)) return false;
  e.preventDefault();
  const option = e.target.closest('[role="option"]');
  if (option) {
    setActive(Number(option.dataset.index));
    pickActive();
  }
  return true;
}

/* Select ------------------------------------------------------------------------------------- */

/** The native element under a Zeyn control, kept for forms and for pages without JavaScript. */
function hideNative(el) {
  el.classList.add('z-native');
  el.tabIndex = -1;
  el.setAttribute('aria-hidden', 'true');
}

/** Turns `[data-zeyn-select]` wrappers into ARIA select only comboboxes. */
function initSelects(root) {
  root.querySelectorAll('.z-select:not([data-ready])').forEach((wrap) => {
    const native = wrap.querySelector('select');
    if (!native) return;
    wrap.dataset.ready = '';
    const combo = make('div', 'z-select-button', {
      role: 'combobox',
      tabindex: native.disabled ? '-1' : '0',
      'aria-haspopup': 'listbox',
      'aria-expanded': 'false',
      'aria-labelledby': `${native.id}-label`,
      'data-testid': `${native.dataset.testid ?? ''}.combobox`,
    });
    for (const a of ['aria-describedby', 'aria-required', 'aria-invalid']) if (native.hasAttribute(a)) combo.setAttribute(a, native.getAttribute(a));
    if (native.required) combo.setAttribute('aria-required', 'true');
    combo.append(make('span', 'z-select-value', { id: `${native.id}-value` }), make('span', 'z-select-arrow', { 'aria-hidden': 'true' }));
    wrap.prepend(combo);
    hideNative(native);
    syncSelect(wrap);
  });
}

/** Shows the native select's chosen option in its combobox. */
function syncSelect(wrap) {
  const native = wrap.querySelector('select');
  const value = wrap.querySelector('.z-select-value');
  const option = native.selectedOptions[0];
  value.textContent = option && option.value !== '' ? option.text : wrap.dataset.textNone;
  value.classList.toggle('z-placeholder', !option || option.value === '');
}

/** Options of a native select as listbox items, with option groups. */
function selectItems(native) {
  return [...native.options].map((o) => ({ value: o.value, label: o.text, group: o.parentElement.tagName === 'OPTGROUP' ? o.parentElement.label : null, selected: o.selected }));
}

/** Opens the list of a select and makes `where` (first, last, selected) active. */
function openSelect(combo, where = 'selected') {
  const wrap = combo.closest('.z-select');
  const native = wrap.querySelector('select');
  openListbox(combo, {
    label: document.getElementById(`${native.id}-label`)?.textContent.replace('*', '').trim() ?? '',
    pick: (item) => {
      native.value = item.value;
      native.dispatchEvent(new Event('input', { bubbles: true }));
      native.dispatchEvent(new Event('change', { bubbles: true }));
      syncSelect(wrap);
      closePopup(false);
      combo.focus();
    },
  });
  const items = selectItems(native);
  fillListbox(items, native.value);
  if (where === 'first') setActive(0);
  if (where === 'last') setActive(items.length - 1);
}

let typed = '';
let typedAt = 0;

/** Type to jump: letters typed quickly move to the next option that starts with them. */
function typeAhead(e, items, from) {
  if (e.key.length !== 1 || e.key === ' ' || e.ctrlKey || e.metaKey || e.altKey || !items.length) return -1;
  const now = Date.now();
  typed = now - typedAt > 600 ? e.key : typed + e.key;
  typedAt = now;
  const repeat = [...typed].every((c) => c === typed[0]);
  const needle = fold(repeat ? typed[0] : typed);
  const offset = repeat || typed.length === 1 ? 1 : 0;
  for (let k = 0; k < items.length; k++) {
    const i = (from + offset + k) % items.length;
    if (fold(items[i].label).startsWith(needle)) return i;
  }
  return -1;
}

/** Keyboard of a select combobox (ARIA select only combobox pattern). */
function selectKey(e, combo) {
  const open = lb?.combo === combo;
  if (!open) {
    if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key) || (e.altKey && e.key === 'ArrowDown')) {
      e.preventDefault();
      openSelect(combo);
      return true;
    }
    if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      openSelect(combo, e.key === 'Home' ? 'first' : 'last');
      return true;
    }
    const native = combo.closest('.z-select').querySelector('select');
    const items = selectItems(native);
    const at = typeAhead(e, items, Math.max(0, native.selectedIndex));
    if (at < 0) return false;
    e.preventDefault();
    openSelect(combo);
    setActive(at);
    return true;
  }
  if (e.key === 'Enter' || e.key === ' ' || (e.altKey && e.key === 'ArrowUp')) {
    e.preventDefault();
    return pickActive();
  }
  if (e.key === 'Tab') {
    pickActive();
    return false;
  }
  if (moveActive(e)) return true;
  const at = typeAhead(e, lb.items, Math.max(0, lb.active));
  if (at < 0) return false;
  e.preventDefault();
  setActive(at);
  return true;
}

/* Listbox control ---------------------------------------------------------------------------- */

/** Turns `[data-zeyn-listbox]` selects into always open listboxes with a search box. */
function initListboxes(root) {
  root.querySelectorAll('.z-listbox-field:not([data-ready])').forEach((wrap) => {
    const native = wrap.querySelector('select');
    if (!native) return;
    wrap.dataset.ready = '';
    const id = `${native.id}-list`;
    const list = make('div', 'z-listbox z-listbox-inline', { role: 'listbox', id, tabindex: '0', 'aria-labelledby': `${native.id}-label` });
    for (const a of ['aria-describedby', 'aria-required', 'aria-invalid']) if (native.hasAttribute(a)) list.setAttribute(a, native.getAttribute(a));
    [...native.options].forEach((o, i) => {
      const el = make('div', 'z-option', { role: 'option', id: `${id}-o${i}`, 'aria-selected': String(o.selected), 'data-value': o.value });
      el.append(make('span', 'z-option-text', { text: o.text }));
      list.append(el);
    });
    if (native.options.length > 8) {
      const search = make('input', 'z-list-search', { type: 'search', 'aria-label': wrap.dataset.textSearch, 'aria-controls': id, autocomplete: 'off', placeholder: wrap.dataset.textSearch });
      wrap.prepend(search);
    }
    wrap.append(list);
    hideNative(native);
    const selected = list.querySelector('[aria-selected="true"]') ?? list.querySelector('[role="option"]');
    if (selected) list.setAttribute('aria-activedescendant', selected.id);
  });
}

/** Chooses an option of an inline listbox (selection follows focus). */
function chooseInline(list, option) {
  if (!option) return;
  const native = list.closest('.z-listbox-field').querySelector('select');
  list.querySelectorAll('[role="option"]').forEach((o) => o.setAttribute('aria-selected', String(o === option)));
  list.setAttribute('aria-activedescendant', option.id);
  reveal(list, option);
  native.value = option.dataset.value;
  native.dispatchEvent(new Event('input', { bubbles: true }));
  native.dispatchEvent(new Event('change', { bubbles: true }));
}

/** Keyboard of an inline listbox: arrows, Home, End, Page keys and type to jump. */
function inlineKey(e, list) {
  const options = [...list.querySelectorAll('[role="option"]:not([hidden])')];
  const current = options.indexOf(document.getElementById(list.getAttribute('aria-activedescendant')));
  const to = { ArrowDown: current + 1, ArrowUp: current - 1, Home: 0, End: options.length - 1, PageDown: current + 8, PageUp: current - 8 }[e.key];
  if (to !== undefined) {
    e.preventDefault();
    chooseInline(list, options[Math.max(0, Math.min(options.length - 1, to))]);
    return true;
  }
  const at = typeAhead(e, options.map((o) => ({ label: o.textContent })), Math.max(0, current));
  if (at < 0) return false;
  e.preventDefault();
  chooseInline(list, options[at]);
  return true;
}

/** Filters an inline listbox by the search text. */
function filterInline(search) {
  const list = search.closest('.z-listbox-field').querySelector('[role="listbox"]');
  const needle = fold(search.value.trim());
  list.querySelectorAll('[role="option"]').forEach((o) => (o.hidden = !!needle && !fold(o.textContent).includes(needle)));
}

/* Lookup ------------------------------------------------------------------------------------- */

/** Turns `[data-zeyn-lookup]` id inputs into searchable comboboxes. */
function initLookups(root) {
  root.querySelectorAll('[data-zeyn-lookup]:not([data-ready])').forEach((wrap) => {
    const value = wrap.querySelector('.z-lookup-value');
    if (!value) return;
    wrap.dataset.ready = '';
    const t = texts(wrap);
    const id = value.id;
    const box = make('div', 'z-lookup-box');
    const tags = make('ul', 'z-tags', { 'aria-label': t.selected });
    const combo = make('input', 'z-lookup-input', {
      type: 'text',
      role: 'combobox',
      id,
      autocomplete: 'off',
      'aria-autocomplete': 'list',
      'aria-expanded': 'false',
      'aria-describedby': value.getAttribute('aria-describedby') ?? '',
      'data-testid': `${value.dataset.testid ?? ''}.combobox`,
    });
    if (value.required) combo.setAttribute('aria-required', 'true');
    if (value.readOnly) combo.readOnly = true;
    value.removeAttribute('id');
    hideNative(value);
    const toggle = make('button', 'z-lookup-toggle', { type: 'button', tabindex: '-1', 'aria-label': t.open });
    toggle.append(make('span', 'z-select-arrow', { 'aria-hidden': 'true' }));
    box.append(tags, combo, toggle);
    wrap.append(box, make('span', 'z-sr-only', { 'aria-live': 'polite' }));
    wrap.dataset.zeynLive = '';
    wrap.zeynLabels = new Map();
    const ids = idsOf(wrap);
    if (ids.length) {
      fetchData(wrap, wrap.dataset.zeynLookup, { ids: ids.map(Number), limit: ids.length }).then((page) => {
        for (const item of page?.items ?? []) wrap.zeynLabels.set(String(item.id), item.label);
        renderLookup(wrap);
      });
    }
  });
}

/** Ids held by a lookup, as strings. */
function idsOf(wrap) {
  return wrap.querySelector('.z-lookup-value').value.split(',').map((s) => s.trim()).filter(Boolean);
}

/** Writes ids into the submitted input and shows them: tags for several, the label for one. */
function setIds(wrap, ids) {
  const value = wrap.querySelector('.z-lookup-value');
  value.value = ids.join(',');
  value.dispatchEvent(new Event('input', { bubbles: true }));
  value.dispatchEvent(new Event('change', { bubbles: true }));
  renderLookup(wrap);
}

/** Draws the chosen values of a lookup. */
function renderLookup(wrap) {
  const t = texts(wrap);
  const combo = wrap.querySelector('[role="combobox"]');
  const ids = idsOf(wrap);
  const label = (id) => wrap.zeynLabels.get(id) ?? `#${id}`;
  if (wrap.dataset.multiple === 'true') {
    const tags = wrap.querySelector('.z-tags');
    tags.replaceChildren(
      ...ids.map((id) => {
        const li = make('li', 'z-tag');
        li.append(make('span', '', { text: label(id) }));
        const remove = make('button', 'z-tag-remove', { type: 'button', 'data-remove': id, 'aria-label': fill(t.remove, { label: label(id) }) });
        remove.append(make('span', '', { 'aria-hidden': 'true', text: '×' }));
        li.append(remove);
        return li;
      }),
    );
  } else {
    combo.value = ids.length ? label(ids[0]) : '';
    combo.dataset.shown = combo.value;
  }
}

const lookupTimers = new WeakMap();

/** Searches the server for what was typed and shows a page of options. */
async function searchLookup(wrap, offset = 0, keep = []) {
  const combo = wrap.querySelector('[role="combobox"]');
  const t = texts(wrap);
  const multiple = wrap.dataset.multiple === 'true';
  const text = multiple || combo.value !== combo.dataset.shown ? combo.value.trim() : '';
  if (lb?.combo !== combo) {
    openListbox(combo, {
      label: document.getElementById(`${combo.id}-label`)?.textContent.replace('*', '').trim() ?? '',
      multiple,
      anchor: wrap.querySelector('.z-lookup-box'),
      pick: (item) => pickLookup(wrap, item),
      more: () => searchLookup(wrap, lb.next, lb.items.filter((i) => !i.more)),
    });
    lookupColumns(wrap);
    fillListbox([...keep, { note: t.loading }]);
  }
  wrap.zeynAbort?.abort();
  wrap.zeynAbort = new AbortController();
  const page = await fetchData(wrap, wrap.dataset.zeynLookup, { text, offset, limit: 20 }, wrap.zeynAbort.signal);
  if (!page || lb?.combo !== combo) return;
  const chosen = new Set(idsOf(wrap));
  const items = [...keep];
  for (const item of page.items ?? []) {
    wrap.zeynLabels.set(String(item.id), item.label);
    items.push({ value: String(item.id), label: item.label, hint: item.hint, cells: item.cells, selected: chosen.has(String(item.id)) });
  }
  lb.next = page.next ? Number(page.next) : null;
  if (page.next) items.push({ value: '', label: t.more, more: true });
  if (!items.length) items.push({ note: t.no_results });
  const active = keep.length ? items[keep.length]?.value : undefined;
  fillListbox(items, active);
  announce(wrap, page.total != null ? fill(t.results, { count: page.total }) : '');
}

/** Picks an option of a lookup: replaces the value, or toggles it when several are allowed. */
function pickLookup(wrap, item) {
  const combo = wrap.querySelector('[role="combobox"]');
  if (wrap.dataset.multiple === 'true') {
    const ids = idsOf(wrap);
    const next = ids.includes(item.value) ? ids.filter((i) => i !== item.value) : [...ids, item.value];
    setIds(wrap, next);
    item.selected = !item.selected;
    item.el.setAttribute('aria-selected', String(item.selected));
    combo.focus();
    return;
  }
  setIds(wrap, [item.value]);
  closePopup(false);
  combo.focus();
}

/** Keyboard of a lookup combobox (ARIA combobox with list autocomplete). */
function lookupKey(e, combo) {
  const wrap = combo.closest('[data-zeyn-lookup]');
  const open = lb?.combo === combo;
  if (!open && (e.key === 'ArrowDown' || (e.altKey && e.key === 'ArrowDown'))) {
    e.preventDefault();
    searchLookup(wrap);
    return true;
  }
  if (!open) {
    if (e.key === 'Backspace' && wrap.dataset.multiple === 'true' && !combo.value) {
      const ids = idsOf(wrap);
      if (ids.length) {
        e.preventDefault();
        setIds(wrap, ids.slice(0, -1));
        return true;
      }
    }
    return false;
  }
  if (e.key === 'Enter') {
    e.preventDefault();
    return pickActive();
  }
  if (e.key === 'ArrowUp' && e.altKey) {
    e.preventDefault();
    closePopup(false);
    return true;
  }
  if (e.key === 'Home' || e.key === 'End') return false;
  return moveActive(e);
}

/** Restores a single lookup's label when the user leaves it without choosing; empty clears it. */
function leaveLookup(combo) {
  const wrap = combo.closest('[data-zeyn-lookup]');
  if (!wrap || wrap.dataset.multiple === 'true') {
    if (wrap) combo.value = '';
    return;
  }
  if (!combo.value.trim() && idsOf(wrap).length) setIds(wrap, []);
  else combo.value = combo.dataset.shown ?? '';
}

/* Numbers ------------------------------------------------------------------------------------ */

/** Reads a number typed in the field's locale; `null` when empty or unreadable. */
function readNumber(wrap, text) {
  const d = wrap.dataset;
  const plain = text.split(d.group).join('').replace(/\s/g, '').replace(d.decimal, '.');
  if (!plain || plain === '-' || !/^-?\d*\.?\d*$/.test(plain)) return null;
  return Number(plain);
}

/** Writes a number in the field's locale: groups and the field's decimals. */
function writeNumber(wrap, n) {
  const d = wrap.dataset;
  const scale = Number(d.scale);
  const [whole, frac] = Math.abs(n).toFixed(scale).split('.');
  const grouped = d.grouped === 'true' ? whole.replace(/\B(?=(\d{3})+(?!\d))/g, d.group) : whole;
  return `${n < 0 ? '-' : ''}${grouped}${frac ? d.decimal + frac : ''}`;
}

/** Formats while typing: adds group separators and keeps at most the field's decimals. */
function formatNumber(input) {
  const wrap = input.closest('[data-zeyn-number]');
  const d = wrap.dataset;
  const scale = Number(d.scale);
  const v = input.value;
  const caret = input.selectionStart ?? v.length;
  const keep = (s) => s.replace(new RegExp(`[^0-9${scale ? d.decimal.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') : ''}-]`, 'g'), '');
  const before = keep(v.slice(0, caret)).length;
  let clean = keep(v);
  const negative = clean.startsWith('-');
  clean = clean.replace(/-/g, '');
  let [whole, ...rest] = clean.split(d.decimal);
  let frac = rest.join('');
  if (scale) frac = frac.slice(0, scale);
  whole = whole.replace(/^0+(?=\d)/, '');
  const grouped = d.grouped === 'true' ? whole.replace(/\B(?=(\d{3})+(?!\d))/g, d.group) : whole;
  const text = `${negative ? '-' : ''}${grouped}${clean.includes(d.decimal) && scale ? d.decimal + frac : ''}`;
  if (text === v) return;
  input.value = text;
  let seen = 0;
  let pos = 0;
  while (pos < text.length && seen < before) {
    if (text[pos] !== d.group) seen++;
    pos++;
  }
  input.setSelectionRange(pos, pos);
  const n = readNumber(wrap, text);
  if (n !== null) input.setAttribute('aria-valuenow', String(n));
  else input.removeAttribute('aria-valuenow');
}

/** Steps a number field by whole units, inside its `min` and `max`. */
function stepNumber(input, by) {
  if (input.readOnly || input.disabled) return;
  const wrap = input.closest('[data-zeyn-number]');
  const d = wrap.dataset;
  let n = (readNumber(wrap, input.value) ?? 0) + by;
  if (d.min !== undefined) n = Math.max(Number(d.min), n);
  if (d.max !== undefined) n = Math.min(Number(d.max), n);
  input.value = writeNumber(wrap, n);
  input.setAttribute('aria-valuenow', String(n));
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
}

/** Arrow Up and Down step by one, Page Up and Down by ten (ARIA spinbutton). */
function numberKey(e, input) {
  const by = { ArrowUp: 1, ArrowDown: -1, PageUp: 10, PageDown: -10 }[e.key];
  if (by === undefined || e.altKey) return false;
  e.preventDefault();
  stepNumber(input, by);
  return true;
}

/* Text areas --------------------------------------------------------------------------------- */

/** Grows a text area with its text (where CSS `field-sizing` is not supported) and updates its counter. */
function textareaInput(area) {
  if (!CSS.supports('field-sizing', 'content')) {
    area.style.height = 'auto';
    area.style.height = `${area.scrollHeight + 2}px`;
  }
  const counter = area.id && document.getElementById(`${area.id}-count`);
  if (!counter) return;
  const max = Number(counter.dataset.zeynCounter);
  const n = [...area.value].length;
  counter.textContent = fill(counter.dataset.text, { n, max });
  const near = n >= max * 0.9;
  counter.classList.toggle('z-near', near);
  if (near) counter.setAttribute('aria-live', 'polite');
  else counter.removeAttribute('aria-live');
}

/** Sets up text areas that grow with their text. */
function initTextareas(root) {
  if (CSS.supports('field-sizing', 'content')) return;
  root.querySelectorAll('[data-zeyn-autosize]').forEach(textareaInput);
}

/* Lines -------------------------------------------------------------------------------------- */

/** Renames the cells of every line after an add, remove or move, so names stay `{field}.{row}.{column}`. */
function renumberLines(wrap) {
  const name = wrap.dataset.zeynLines;
  const heads = [...wrap.querySelectorAll('thead th')].map((th) => th.textContent.trim());
  const base = wrap.querySelector(`input[name="${CSS.escape(name)}"]`)?.id ?? name;
  const rows = [...wrap.querySelectorAll('tbody > tr[data-zeyn-line]')];
  rows.forEach((tr, i) => {
    const n = i + 1;
    const rowText = fill(wrap.dataset.textRow, { n });
    tr.querySelector('.z-lines-n').firstChild.textContent = String(n);
    tr.querySelectorAll('input[name]').forEach((input) => {
      const col = input.name.split('.').pop();
      input.name = `${name}.${i}.${col}`;
      if (!input.dataset.col) return;
      const id = `${base}-${i}-${col}`;
      const error = input.nextElementSibling;
      input.id = id;
      input.setAttribute('aria-describedby', `${id}-error`);
      if (error) error.id = `${id}-error`;
      const cellIndex = [...tr.children].indexOf(input.closest('td'));
      input.setAttribute('aria-label', `${heads[cellIndex]}, ${rowText}`);
    });
    tr.querySelector('[data-zeyn-line-move="-1"]')?.setAttribute('aria-label', fill(wrap.dataset.textUp, { n }));
    tr.querySelector('[data-zeyn-line-move="1"]')?.setAttribute('aria-label', fill(wrap.dataset.textDown, { n }));
    tr.querySelector('[data-zeyn-line-remove]')?.setAttribute('aria-label', fill(wrap.dataset.textRemove, { n }));
  });
  wrap.querySelector('[data-zeyn-lines-empty]').hidden = rows.length > 0;
  sumLines(wrap);
}

/** Reads money typed in the line editor's locale as minor units; `null` when unreadable. */
function minorUnits(wrap, text, scale) {
  const d = wrap.dataset;
  const plain = text.split(d.group).join('').replace(/\s/g, '').replace(d.decimal, '.');
  if (!/^-?\d+(\.\d+)?$/.test(plain)) return null;
  const [whole, frac = ''] = plain.replace('-', '').split('.');
  if (frac.length > scale) return null;
  const value = BigInt(whole) * 10n ** BigInt(scale) + BigInt(frac.padEnd(scale, '0') || '0');
  return plain.startsWith('-') ? -value : value;
}

/** Writes minor units as an amount in the line editor's locale. */
function writeMinor(wrap, minor) {
  const d = wrap.dataset;
  const negative = minor < 0n;
  const abs = (negative ? -minor : minor).toString().padStart(3, '0');
  const whole = abs.slice(0, -2).replace(/\B(?=(\d{3})+(?!\d))/g, d.group);
  return `${negative ? '-' : ''}${whole}${d.decimal}${abs.slice(-2)}`;
}

/** Updates every line total and the footer total from what is typed. Display only: the server works them out again. */
function sumLines(wrap) {
  const sumCell = wrap.querySelector('[data-zeyn-lines-sum]');
  if (!sumCell) return;
  let sum = 0n;
  wrap.querySelectorAll('tbody > tr[data-zeyn-line]').forEach((tr) => {
    const qty = minorUnits(wrap, tr.querySelector('[data-zeyn-qty]')?.value ?? '', 0);
    const price = minorUnits(wrap, tr.querySelector('[data-zeyn-price]')?.value ?? '', 2);
    const cell = tr.querySelector('[data-zeyn-line-total]');
    if (qty === null || price === null) {
      if (cell) cell.textContent = '';
      return;
    }
    const total = qty * price;
    sum += total;
    if (cell) cell.textContent = writeMinor(wrap, total);
  });
  sumCell.textContent = writeMinor(wrap, sum);
}

/** Adds an empty line at the end and moves focus into it. */
function addLine(wrap) {
  const template = wrap.querySelector('template[data-zeyn-line-template]');
  const row = template.content.firstElementChild.cloneNode(true);
  wrap.querySelector('tbody').append(row);
  renumberLines(wrap);
  row.querySelector('input[data-col]:not([readonly])')?.focus();
}

/** Removes a line; focus goes to the same place in the next line, or to the add button. */
function removeLine(button) {
  const wrap = button.closest('[data-zeyn-lines]');
  const tr = button.closest('tr');
  const next = tr.nextElementSibling ?? tr.previousElementSibling;
  tr.remove();
  renumberLines(wrap);
  (next?.querySelector('[data-zeyn-line-remove]') ?? wrap.querySelector('[data-zeyn-line-add]')).focus();
}

/** Moves a line up or down, keeping focus on the button that moved it. */
function moveLine(button) {
  const wrap = button.closest('[data-zeyn-lines]');
  const tr = button.closest('tr');
  const by = Number(button.dataset.zeynLineMove);
  const other = by < 0 ? tr.previousElementSibling : tr.nextElementSibling;
  if (!other) return;
  if (by < 0) other.before(tr);
  else other.after(tr);
  renumberLines(wrap);
  button.focus();
}

/** Puts the server's line errors in their cells and keeps only other messages under the table. */
function showLineErrors(form, errors) {
  let first = null;
  for (const wrap of form.querySelectorAll('[data-zeyn-lines]')) {
    const name = wrap.dataset.zeynLines;
    const box = document.getElementById(`${wrap.querySelector(`input[name="${CSS.escape(name)}"]`)?.id}-error`);
    const own = errors.filter((e) => e.field === name);
    if (!own.length) continue;
    for (const e of own.filter((x) => x.rule?.rule === 'line')) {
      const input = wrap.querySelector(`[name="${CSS.escape(`${name}.${e.rule.value.row}.${e.rule.value.field}`)}"]`);
      if (!input) continue;
      input.setAttribute('aria-invalid', 'true');
      const cell = document.getElementById(`${input.id}-error`);
      if (cell) cell.textContent = e.message;
      first ??= input;
    }
    if (box) box.textContent = own.filter((x) => x.rule?.rule !== 'line').map((x) => x.message).join(' ');
  }
  first?.focus();
}

/* Confirm, alerts, breadcrumbs --------------------------------------------------------------- */

/**
 * Asks the user to confirm in a modal alert dialog. Cancel has the focus first, so Enter alone
 * never runs a destructive action. Resolves `true` only for the confirm button.
 * @param {{title: string, message: string, ok: string, cancel: string, danger?: boolean}} t
 */
function confirmDialog(t) {
  return new Promise((resolve) => {
    const opener = document.activeElement;
    const id = `z-confirm-${++listUid}`;
    const dialog = make('dialog', 'z-confirm', { role: 'alertdialog', 'aria-labelledby': `${id}-title`, 'aria-describedby': `${id}-text` });
    const cancel = make('button', 'z-button', { type: 'button', text: t.cancel });
    const ok = make('button', `z-button ${t.danger ? 'z-danger-button' : 'z-primary'}`, { type: 'button', text: t.ok });
    const actions = make('div', 'z-confirm-actions');
    actions.append(cancel, ok);
    dialog.append(make('h2', '', { id: `${id}-title`, text: t.title }), make('p', '', { id: `${id}-text`, text: t.message }), actions);
    let answered = false;
    const finish = (yes) => {
      if (answered) return;
      answered = true;
      if (dialog.open) dialog.close();
      dialog.remove();
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
      resolve(yes);
    };
    cancel.addEventListener('click', () => finish(false));
    ok.addEventListener('click', () => finish(true));
    dialog.addEventListener('cancel', (e) => {
      e.preventDefault();
      finish(false);
    });
    document.body.append(dialog);
    dialog.showModal();
    cancel.focus();
  });
}

/** Asks before submitting forms that carry `data-zeyn-confirm` (action buttons). */
async function confirmSubmit(e) {
  const form = e.target;
  if (!(form instanceof HTMLFormElement) || !form.dataset.zeynConfirm) return;
  if (form.zeynConfirmed) {
    form.zeynConfirmed = false;
    return;
  }
  e.preventDefault();
  e.stopImmediatePropagation();
  const d = form.dataset;
  const danger = !!form.querySelector('.z-danger-button');
  if (await confirmDialog({ title: d.textTitle, message: d.zeynConfirm, ok: d.textOk, cancel: d.textCancel, danger })) {
    form.zeynConfirmed = true;
    form.requestSubmit();
  }
}

/** Prefixes app relative links (breadcrumbs) with the path the app is served under. */
function baseLinks(root) {
  root.querySelectorAll('a[data-zeyn-base-href]').forEach((a) => {
    a.setAttribute('href', BASE + a.getAttribute('href'));
    a.removeAttribute('data-zeyn-base-href');
  });
}

/* Steps -------------------------------------------------------------------------------------- */

/** Form values as the runtime sends them: booleans for checkboxes, strings otherwise. */
function valuesOf(form) {
  const values = {};
  for (const el of form.elements) {
    if (!el.name || el.disabled || el.readOnly) continue;
    if (el.type === 'hidden' && !el.hasAttribute('data-zeyn-version')) continue;
    if (el.matches('button, fieldset, output, [type="submit"], [type="button"], [type="reset"]')) continue;
    if (el.type === 'checkbox') values[el.name] = el.checked;
    else if (el.type === 'radio') {
      if (el.checked) values[el.name] = el.value;
    } else if (el.multiple) values[el.name] = [...el.selectedOptions].map((o) => o.value).join(',');
    else values[el.name] = el.value;
  }
  return values;
}

/** Shows one step of a stepped form, updates the step list and the buttons, and announces it. */
function showStep(form, n, focus = true) {
  const steps = [...form.querySelectorAll('fieldset[data-zeyn-step]')];
  const list = form.querySelector('[data-zeyn-steps]');
  const total = steps.length;
  n = Math.max(0, Math.min(total - 1, n));
  form.dataset.step = String(n);
  steps.forEach((s, i) => (s.hidden = i !== n));
  list?.querySelectorAll('li').forEach((li, i) => {
    if (i === n) li.setAttribute('aria-current', 'step');
    else li.removeAttribute('aria-current');
    li.classList.toggle('z-step-done', i < n);
  });
  const back = form.querySelector('[data-zeyn-step-back]');
  const next = form.querySelector('[data-zeyn-step-next]');
  const submit = form.querySelector('.z-form-actions [type="submit"]');
  if (back) back.hidden = n === 0;
  if (next) next.hidden = n === total - 1;
  if (submit) submit.hidden = n !== total - 1;
  if (list) announce(form, fill(list.dataset.textProgress, { n: n + 1, total }));
  if (focus) (steps[n].querySelector('input:not([type="hidden"]):not(.z-native), select:not(.z-native), textarea, [role="combobox"], [role="listbox"]') ?? steps[n]).focus();
}

/** Sets up stepped forms: one step at a time, with the step buttons shown. */
function initSteps(root) {
  root.querySelectorAll('form:has([data-zeyn-steps]):not([data-step])').forEach((form) => {
    form.dataset.zeynLive = '';
    if (!form.querySelector(':scope > .z-sr-only[aria-live]')) form.append(make('span', 'z-sr-only', { 'aria-live': 'polite' }));
    showStep(form, 0, false);
  });
}

/** Writes errors under their fields, as the runtime does, and focuses the first one. */
function showFieldErrors(form, errors) {
  const scope = form.closest('[data-zeyn-screen]')?.dataset.zeynScreen;
  form.querySelectorAll('.z-field-error').forEach((p) => (p.textContent = ''));
  form.querySelectorAll('[aria-invalid="true"]').forEach((el) => el.removeAttribute('aria-invalid'));
  let first = null;
  for (const err of errors) {
    const named = form.elements.namedItem(err.field);
    const input = named instanceof RadioNodeList ? named[0] : named;
    const box = document.getElementById(input?.id ? `${input.id}-error` : `${scope}-${err.field}-error`) ?? document.getElementById(`${scope}-${err.field}-error`);
    if (box) box.textContent = box.textContent ? `${box.textContent} ${err.message}` : err.message;
    input?.setAttribute?.('aria-invalid', 'true');
    first ??= input;
  }
  first?.focus?.();
}

/** Next: checks the step's fields on the server, then moves on when they are fine. */
async function nextStep(button) {
  const form = button.closest('form');
  const n = Number(form.dataset.step);
  button.disabled = true;
  const data = await fetchData(form, '_step', { step: n, values: valuesOf(form) });
  button.disabled = false;
  const errors = data?.errors ?? [];
  if (errors.length) {
    showFieldErrors(form, errors);
    return;
  }
  form.querySelectorAll('.z-field-error').forEach((p) => (p.textContent = ''));
  showStep(form, n + 1);
}

/** After a failed save, shows the step that holds the first error. */
function stepOfErrors(form, errors) {
  if (!form.matches('[data-step]')) return;
  for (const err of errors) {
    const named = form.elements.namedItem(err.field);
    const input = named instanceof RadioNodeList ? named[0] : named;
    const step = input?.closest('fieldset[data-zeyn-step]');
    if (step) {
      showStep(form, Number(step.dataset.zeynStep), false);
      (form.querySelector('[aria-invalid="true"]:not(.z-native)') ?? input).focus();
      return;
    }
  }
}

/* Tree --------------------------------------------------------------------------------------- */

/** Builds the tree items of one level from the action's items. */
function treeItems(wrap, items, level) {
  const max = Number(wrap.dataset.max) || 1000;
  const list = items.slice(0, max);
  return list.map((item, i) => {
    const li = make('li', 'z-tree-item', {
      role: 'treeitem',
      id: `${wrap.zeynId}-${++listUid}`,
      tabindex: '-1',
      'aria-level': String(level),
      'aria-setsize': String(list.length),
      'aria-posinset': String(i + 1),
      'aria-selected': 'false',
      'data-id': item.id,
    });
    li.style.setProperty('--z-level', String(level - 1));
    if (item.has_children) {
      li.setAttribute('aria-expanded', 'false');
      li.dataset.lazy = '';
    }
    const row = make('span', 'z-tree-row');
    row.append(make('span', 'z-tree-toggle', { 'aria-hidden': 'true' }), make('span', 'z-tree-label', { text: item.label }));
    if (item.hint) row.append(make('span', 'z-tree-hint', { text: item.hint }));
    li.append(row);
    return li;
  });
}

/** Loads one level of a tree into `list` (the tree or a group). */
async function loadTree(wrap, list, parent, level) {
  const t = texts(wrap);
  list.setAttribute('aria-busy', 'true');
  const items = await fetchData(wrap, '', { parent });
  list.removeAttribute('aria-busy');
  if (!items) {
    list.replaceChildren(make('li', 'z-tree-note', { role: 'none', text: t.failed }));
    return false;
  }
  const nodes = treeItems(wrap, items, level);
  list.replaceChildren(...(nodes.length ? nodes : [make('li', 'z-tree-note', { role: 'none', text: t.empty })]));
  return true;
}

/** Loads the top level of every tree inside `root`. */
function initTrees(root) {
  root.querySelectorAll('[data-zeyn-tree]:not([data-ready])').forEach(async (wrap) => {
    wrap.dataset.ready = '';
    wrap.zeynId = `z-tree-${++listUid}`;
    const tree = wrap.querySelector('[role="tree"]');
    await loadTree(wrap, tree, null, 1);
    const first = tree.querySelector('[role="treeitem"]');
    if (first) first.tabIndex = 0;
  });
}

/** Visible tree items in order. */
function visibleItems(tree) {
  return [...tree.querySelectorAll('[role="treeitem"]')].filter((li) => !li.parentElement.closest('[role="treeitem"][aria-expanded="false"]'));
}

/** Moves the roving focus to a tree item. */
function focusItem(li) {
  const tree = li.closest('[role="tree"]');
  tree.querySelectorAll('[role="treeitem"][tabindex="0"]').forEach((x) => (x.tabIndex = -1));
  li.tabIndex = 0;
  li.focus();
}

/** Opens or closes a branch, loading its children the first time. */
async function toggleItem(li, open) {
  if (!li.hasAttribute('aria-expanded')) return;
  const want = open ?? li.getAttribute('aria-expanded') !== 'true';
  li.setAttribute('aria-expanded', String(want));
  if (want && li.dataset.lazy !== undefined) {
    delete li.dataset.lazy;
    const group = make('ul', 'z-tree-group', { role: 'group' });
    li.append(group);
    li.setAttribute('aria-busy', 'true');
    group.append(make('li', 'z-tree-note', { role: 'none', text: texts(li.closest('[data-zeyn-tree]')).loading }));
    const ok = await loadTree(li.closest('[data-zeyn-tree]'), group, li.dataset.id, Number(li.getAttribute('aria-level')) + 1);
    li.removeAttribute('aria-busy');
    if (!ok) li.dataset.lazy = '';
  }
}

/** Selects one item (single selection follows Enter, Space and clicks). */
function selectItem(li) {
  const tree = li.closest('[role="tree"]');
  tree.querySelectorAll('[aria-selected="true"]').forEach((x) => x.setAttribute('aria-selected', 'false'));
  li.setAttribute('aria-selected', 'true');
  li.closest('[data-zeyn-tree]').dispatchEvent(new CustomEvent('zeyn:tree-select', { bubbles: true, detail: { id: li.dataset.id } }));
}

/** Keyboard of a tree (ARIA tree pattern). */
function treeKey(e, li) {
  const tree = li.closest('[role="tree"]');
  const items = visibleItems(tree);
  const i = items.indexOf(li);
  const expanded = li.getAttribute('aria-expanded');
  switch (e.key) {
    case 'ArrowDown':
      if (items[i + 1]) focusItem(items[i + 1]);
      break;
    case 'ArrowUp':
      if (items[i - 1]) focusItem(items[i - 1]);
      break;
    case 'Home':
      focusItem(items[0]);
      break;
    case 'End':
      focusItem(items.at(-1));
      break;
    case 'ArrowRight':
      if (expanded === 'false') toggleItem(li, true);
      else if (expanded === 'true') {
        const child = li.querySelector(':scope > [role="group"] > [role="treeitem"]');
        if (child) focusItem(child);
      }
      break;
    case 'ArrowLeft':
      if (expanded === 'true') toggleItem(li, false);
      else {
        const parent = li.parentElement.closest('[role="treeitem"]');
        if (parent) focusItem(parent);
      }
      break;
    case 'Enter':
    case ' ':
      selectItem(li);
      break;
    default: {
      const at = typeAhead(e, items.map((x) => ({ label: x.querySelector('.z-tree-label').textContent })), i);
      if (at < 0) return false;
      focusItem(items[at]);
    }
  }
  e.preventDefault();
  return true;
}

/* Native elements under controls ------------------------------------------------------------- */

/** The visible control of a hidden native element. */
function controlOf(native) {
  const wrap = native.closest('.z-select, .z-listbox-field, [data-zeyn-lookup]');
  return wrap?.querySelector('[role="combobox"], [role="listbox"]') ?? null;
}

/** Validation marks set on a hidden native element (by the runtime) show on its control. */
new MutationObserver((records) => {
  for (const r of records) {
    const el = r.target;
    if (!(el instanceof Element) || !el.classList.contains('z-native')) continue;
    const control = controlOf(el);
    if (!control) continue;
    if (el.getAttribute('aria-invalid') === 'true') control.setAttribute('aria-invalid', 'true');
    else control.removeAttribute('aria-invalid');
  }
}).observe(document.body, { subtree: true, attributes: true, attributeFilter: ['aria-invalid'] });

/* Setup -------------------------------------------------------------------------------------- */

/**
 * Browser modules a page loads only when it holds one of their components, by selector: the
 * catalog (card grids, rails, search boxes), media players and embeds, uploads, charts and chat
 * panels of the assistant, share buttons, filter chips. Each module sets up the whole document when it loads and listens to
 * `zeyn:content` after that, so content that `nav.js` swaps in later is covered too.
 */
const LAZY = [
  ['[data-zeyn-cards], [data-zeyn-searchbox]', 'catalog'],
  ['[data-zeyn-player], .z-embed', 'media'],
  ['.z-upload', 'files'],
  ['[data-zeyn-chart]', 'charts'],
  ['section[data-zeyn-assistant]:not([data-zeyn-assistant="dock"])', 'assistant'],
  ['button[data-zeyn-share]', 'share'],
  ['a[data-zeyn-chip]', 'chips'],
  ['[data-zeyn-hero]', 'hero'],
];

/** @type {Set<string>} Modules of `LAZY` already asked for. */
const lazyLoaded = new Set();

/** Imports one module of `LAZY` once. */
function loadLazy(name) {
  if (lazyLoaded.has(name)) return Promise.resolve();
  lazyLoaded.add(name);
  return import(`${BASE}/_zeyn/${name}.js${VERSION}`);
}

/** Loads the modules whose components are inside `root`. */
function initLazy(root) {
  for (const [selector, name] of LAZY) if (!lazyLoaded.has(name) && root.querySelector(selector)) loadLazy(name);
}

/**
 * The assistant's launcher is on every page; its module loads on the first press, which it then
 * handles itself.
 */
document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('[data-zeyn-assistant-launch]') : null;
  if (!button || lazyLoaded.has('assistant')) return;
  loadLazy('assistant').then(() => button.click());
});

/** Initializers run for every root that gets new content. */
const initializers = [restoreTabs, initSelects, initListboxes, initLookups, initTextareas, initSteps, baseLinks, initTrees, initLazy];

/** Initializes every component inside `root`. Safe to call more than once. */
function init(root) {
  for (const fn of initializers) fn(root);
}

/** Keydown handlers, tried in order until one handles the event. */
const keyHandlers = [
  (e) => popupKey(e),
  (e) => pickerKey(e),
  (e, t) => {
    if (!(e.altKey && e.key === 'ArrowDown' && t.matches('[data-zeyn-date-input]'))) return false;
    const root = t.closest('[data-zeyn-date]');
    e.preventDefault();
    openPicker(root, t);
    return true;
  },
  (e, t) => t.matches('[data-zeyn-date-input]') && stepTime(t, e),
  (e, t) => {
    if (e.key !== 'Enter' || !t.matches('[data-zeyn-filter-date]')) return false;
    check(t);
    return false;
  },
  (e, t) => t.matches('.z-select-button') && selectKey(e, t),
  (e, t) => t.matches('[data-zeyn-lookup] [role="combobox"]') && lookupKey(e, t),
  (e, t) => t.matches('.z-listbox-inline') && inlineKey(e, t),
  (e, t) => {
    if (!t.matches('.z-list-search') || !['ArrowDown', 'ArrowUp'].includes(e.key)) return false;
    const list = t.closest('.z-listbox-field').querySelector('[role="listbox"]');
    list.focus();
    return inlineKey(e, list);
  },
  (e, t) => t.matches('[data-zeyn-number] input') && numberKey(e, t),
  (e, t) => t.matches('[data-zeyn-tree] [role="treeitem"]') && treeKey(e, t),
  (e, t) => t.matches('[role="tab"]') && tabKey(e, t),
  (e, t) => t.matches('.z-split-bar') && splitKey(e, t),
];

/** Click handlers, tried in order until one handles the event. */
const clickHandlers = [
  pickerClick,
  (e, t) => {
    const combo = t.closest('.z-select-button');
    if (!combo) return false;
    if (lb?.combo === combo) closePopup(true);
    else openSelect(combo);
    return true;
  },
  (e, t) => {
    const b = t.closest('.z-lookup-toggle');
    if (!b) return false;
    const wrap = b.closest('[data-zeyn-lookup]');
    const combo = wrap.querySelector('[role="combobox"]');
    if (lb?.combo === combo) closePopup(true);
    else {
      combo.focus();
      searchLookup(wrap);
    }
    return true;
  },
  (e, t) => {
    const b = t.closest('[data-remove]');
    if (!b) return false;
    const wrap = b.closest('[data-zeyn-lookup]');
    setIds(wrap, idsOf(wrap).filter((id) => id !== b.dataset.remove));
    wrap.querySelector('[role="combobox"]').focus();
    return true;
  },
  (e, t) => {
    const b = t.closest('[data-zeyn-step]');
    if (!b) return false;
    const input = b.closest('[data-zeyn-number]').querySelector('input');
    stepNumber(input, Number(b.dataset.zeynStep));
    input.focus();
    return true;
  },
  (e, t) => {
    const row = t.closest('[data-zeyn-tree] .z-tree-row');
    if (!row) return false;
    const li = row.parentElement;
    focusItem(li);
    if (t.closest('.z-tree-toggle')) toggleItem(li);
    else {
      selectItem(li);
      if (li.hasAttribute('aria-expanded')) toggleItem(li);
    }
    return true;
  },
  (e, t) => {
    const b = t.closest('[data-zeyn-dismiss]');
    if (!b) return false;
    const heading = b.closest('dialog')?.querySelector('h2') ?? document.querySelector('#z-main h1');
    b.closest('.z-callout')?.remove();
    if (heading) {
      heading.tabIndex = -1;
      heading.focus();
    }
    return true;
  },
  (e, t) => {
    const b = t.closest('[data-zeyn-step-next], [data-zeyn-step-back]');
    if (!b) return false;
    if (b.hasAttribute('data-zeyn-step-next')) nextStep(b);
    else showStep(b.closest('form'), Number(b.closest('form').dataset.step) - 1);
    return true;
  },
  (e, t) => {
    const b = t.closest('[data-zeyn-line-add], [data-zeyn-line-remove], [data-zeyn-line-move]');
    if (!b) return false;
    if (b.hasAttribute('data-zeyn-line-add')) addLine(b.closest('[data-zeyn-lines]'));
    else if (b.hasAttribute('data-zeyn-line-remove')) removeLine(b);
    else moveLine(b);
    return true;
  },
  (e, t) => {
    const option = t.closest('.z-listbox-inline [role="option"]');
    if (!option) return false;
    const list = option.closest('[role="listbox"]');
    chooseInline(list, option);
    list.focus();
    return true;
  },
  (e, t) => {
    const b = t.closest('[data-zeyn-toggle]');
    return b && (togglePanel(b), true);
  },
  (e, t) => {
    const tab = t.closest('[data-zeyn-tabs] [role="tab"]');
    return tab && (selectTab(tab), true);
  },
];

document.addEventListener('click', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (t) clickHandlers.some((h) => h(e, t));
});

document.addEventListener(
  'keydown',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    if (t && !e.defaultPrevented) keyHandlers.some((h) => h(e, t));
  },
  true,
);

document.addEventListener(
  'beforeinput',
  (e) => {
    const t = e.target;
    if (!(t instanceof HTMLInputElement) || !t.matches('[data-zeyn-date-input]')) return;
    const root = t.closest('[data-zeyn-date]');
    const seps = root?.dataset.zeynDate === 'time' ? ':' : `${root?.dataset.sep ?? ''}:`;
    if (e.inputType === 'insertText' && e.data && seps.includes(e.data) && t.value.endsWith(e.data) && t.selectionStart === t.value.length) e.preventDefault();
  },
  true,
);

document.addEventListener(
  'input',
  (e) => {
    const t = e.target;
    if (!(t instanceof HTMLInputElement) || !t.matches('[data-zeyn-date-input]') || own.has(e)) return;
    mask(t, e);
    if (t.matches('[data-zeyn-filter-date]')) {
      e.stopPropagation();
      scheduleCheck(t);
    }
  },
  true,
);

document.addEventListener('input', (e) => {
  const t = e.target;
  if (!(t instanceof Element)) return;
  if (t.matches('[data-zeyn-number] input') && e.inputType) formatNumber(t);
  else if (t.matches('textarea[data-zeyn-autosize]')) textareaInput(t);
  else if (t.matches('.z-list-search')) filterInline(t);
  else if (t.matches('[data-zeyn-lines] input[data-col]')) sumLines(t.closest('[data-zeyn-lines]'));
  else if (t.matches('[data-zeyn-lookup] [role="combobox"]')) {
    const wrap = t.closest('[data-zeyn-lookup]');
    clearTimeout(lookupTimers.get(wrap));
    lookupTimers.set(wrap, setTimeout(() => searchLookup(wrap), LOOKUP_DELAY_MS));
  }
});

document.addEventListener('focusin', (e) => {
  const t = e.target;
  if (t instanceof Element && t.classList.contains('z-native')) controlOf(t)?.focus();
});

document.addEventListener('zeyn:errors', (e) => {
  if (!(e.target instanceof HTMLFormElement)) return;
  showLineErrors(e.target, e.detail ?? []);
  stepOfErrors(e.target, e.detail ?? []);
});

document.addEventListener('submit', confirmSubmit, true);

document.addEventListener('zeyn:effect', async (e) => {
  const { effect, run } = e.detail ?? {};
  if (effect?.effect !== 'confirm') return;
  if (await confirmDialog(effect)) run?.(effect.then);
});

/**
 * Other scripts ask the user through the same dialog: `zeyn:confirm` with
 * `detail = { title, message, ok, cancel, danger, resolve }`. The event is canceled when handled
 * and `resolve(true)` runs only for the confirm button.
 */
document.addEventListener('zeyn:confirm', (e) => {
  const d = e.detail ?? {};
  if (typeof d.resolve !== 'function') return;
  e.preventDefault();
  confirmDialog(d).then(d.resolve);
});

document.addEventListener('focusout', (e) => {
  const t = e.target;
  if (!(t instanceof Element) || !t.matches('[data-zeyn-lookup] [role="combobox"]')) return;
  if (lb?.combo === t) closePopup(false);
  leaveLookup(t);
});

document.addEventListener('change', (e) => {
  const t = e.target;
  if (t instanceof HTMLInputElement && t.matches('[data-zeyn-date-input]') && !picker) check(t);
});

document.addEventListener('focusin', (e) => {
  const t = e.target;
  if (t instanceof HTMLInputElement && t.matches('[data-zeyn-filter-date]')) upgradeFilter(t);
});

document.addEventListener('pointerover', previewRange);
document.addEventListener('focusin', previewRange);

document.addEventListener(
  'pointerdown',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    if (t && listboxPointer(e)) return;
    if (popup && t && !popup.panel.contains(t) && !popup.anchor.contains(t)) closePopup(false);
    const bar = t?.closest('.z-split-bar');
    if (bar && e.button === 0) dragSplit(e, bar);
  },
  true,
);

window.addEventListener('resize', placePopup);
document.addEventListener('scroll', (e) => popup && !popup.panel.contains(e.target) && placePopup(), true);
NARROW.addEventListener('change', placePopup);
window.addEventListener('hashchange', () => restoreTabs(document));

document.addEventListener('zeyn:leave', () => closePopup(false));
document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));

new MutationObserver((records) => {
  for (const r of records) for (const n of r.addedNodes) if (n instanceof HTMLDialogElement) init(n);
}).observe(document.body, { childList: true });

init(document);

/* Masked text -------------------------------------------------------------------------------- */

/** Slots and literals of a mask pattern: `9` digit, `a` letter, `A` upper case letter, `*` either. */
function maskTokens(pattern) {
  const out = [];
  for (let i = 0; i < pattern.length; i++) {
    const c = pattern[i];
    if (c === '\\' && i + 1 < pattern.length) out.push({ lit: pattern[++i] });
    else if ('9aA*'.includes(c)) out.push({ slot: c });
    else out.push({ lit: c });
  }
  return out;
}

/** Whether a slot takes a character. */
function slotTakes(slot, c) {
  if (slot === '9') return /^\d$/.test(c);
  if (slot === '*') return /^[\p{L}\p{N}]$/u.test(c);
  return /^\p{L}$/u.test(c);
}

/** The raw characters of typed text, as the server reads them (`ui::mask_raw`). */
function maskRaw(tokens, text) {
  let raw = '';
  let at = 0;
  outer: for (const c of text) {
    while (tokens[at]?.lit !== undefined) {
      if (tokens[at++].lit === c) continue outer;
    }
    const t = tokens[at];
    if (t && slotTakes(t.slot, c)) {
      raw += t.slot === 'A' ? c.toLocaleUpperCase(lang()) : c;
      at++;
    } else if (/[\p{L}\p{N}]/u.test(c)) break;
  }
  return raw;
}

/** Raw characters shown through the pattern, with the literals up to the last one typed. */
function maskFormat(tokens, raw) {
  let out = '';
  let i = 0;
  for (const t of tokens) {
    if (i >= raw.length) break;
    if (t.lit !== undefined) out += t.lit;
    else out += raw[i++];
  }
  return out;
}

/** Turns `[data-zeyn-mask]` inputs into masked inputs over the raw value the form submits. */
function initMasks(root) {
  root.querySelectorAll('.z-mask:not([data-ready])').forEach((wrap) => {
    const native = wrap.querySelector('.z-mask-value');
    if (!native) return;
    wrap.dataset.ready = '';
    const tokens = maskTokens(wrap.dataset.zeynMask);
    const shown = make('input', 'z-mask-input', {
      type: 'text',
      id: native.id,
      autocomplete: 'off',
      spellcheck: 'false',
      inputmode: native.inputMode || 'text',
      dir: 'ltr',
      placeholder: wrap.dataset.example ?? '',
      'data-testid': `${native.dataset.testid ?? ''}.input`,
    });
    for (const a of ['aria-describedby', 'aria-required', 'aria-invalid']) if (native.hasAttribute(a)) shown.setAttribute(a, native.getAttribute(a));
    shown.readOnly = native.readOnly;
    shown.value = maskFormat(tokens, maskRaw(tokens, native.value));
    native.removeAttribute('id');
    hideNative(native);
    wrap.prepend(shown);
    wrap.zeynTokens = tokens;
  });
}

/**
 * Reformats a masked input as the user types, keeps the caret after the same raw character and
 * writes the raw value. Backspace over a literal removes the character before it.
 */
function maskInput(shown, e) {
  const wrap = shown.closest('.z-mask');
  const tokens = wrap.zeynTokens;
  const native = wrap.querySelector('.z-mask-value');
  const caret = shown.selectionStart ?? shown.value.length;
  let before = maskRaw(tokens, shown.value.slice(0, caret)).length;
  let raw = maskRaw(tokens, shown.value);
  if (e.inputType === 'deleteContentBackward' && raw === native.value && before > 0) {
    raw = raw.slice(0, before - 1) + raw.slice(before);
    before--;
  }
  const text = maskFormat(tokens, raw);
  if (text !== shown.value) {
    shown.value = text;
    let seen = 0;
    let pos = 0;
    for (const t of tokens) {
      if (seen >= before || pos >= text.length) break;
      pos++;
      if (t.lit === undefined) seen++;
    }
    shown.setSelectionRange(pos, pos);
  }
  if (native.value === raw) return;
  native.value = raw;
  native.dispatchEvent(new Event('input', { bubbles: true }));
  native.dispatchEvent(new Event('change', { bubbles: true }));
}

/* Password ----------------------------------------------------------------------------------- */

/** Shows the show and hide buttons of password inputs, which need this script. */
function initPasswords(root) {
  root.querySelectorAll('[data-zeyn-password]:not([data-ready])').forEach((wrap) => {
    wrap.dataset.ready = '';
    const button = wrap.querySelector('.z-password-toggle');
    if (button) button.hidden = false;
  });
}

/** Shows or hides the password (APG toggle button: the name stays, `aria-pressed` changes). */
function togglePassword(button) {
  const input = button.closest('[data-zeyn-password]').querySelector('input');
  const show = button.getAttribute('aria-pressed') !== 'true';
  input.type = show ? 'text' : 'password';
  button.setAttribute('aria-pressed', String(show));
}

/**
 * A rough strength from 1 (weak) to 4 (strong): length, kinds of characters and obvious
 * patterns. Only a hint; the server's rules decide what is accepted.
 */
function strengthOf(pw) {
  const len = [...pw].length;
  const kinds = [/\p{Ll}/u, /\p{Lu}/u, /\p{N}/u, /[^\p{L}\p{N}]/u].filter((r) => r.test(pw)).length;
  if (len < 8 || /^(.)\1*$/u.test(pw) || /^(?:password|parola|qwerty|123456|111111|abc123)/i.test(pw)) return 1;
  return Math.min(4, 1 + (len >= 12) + (kinds >= 3) + (len >= 16 || (len >= 12 && kinds === 4)));
}

/** Updates the strength meter under a new password input. */
function meterInput(input) {
  const meter = document.querySelector(`[data-zeyn-strength="${CSS.escape(input.id)}"]`);
  if (!meter) return;
  const level = input.value ? strengthOf(input.value) : 0;
  meter.hidden = level === 0;
  if (meter.dataset.level === String(level)) return;
  meter.dataset.level = String(level);
  const levels = parse(meter.dataset.levels ?? '[]') ?? [];
  meter.querySelector('.z-strength-text').textContent = level ? (levels[level - 1] ?? '') : '';
}

/* Setup of the inputs above ------------------------------------------------------------------ */

initializers.push(initMasks, initPasswords);

clickHandlers.push((e, t) => {
  const b = t.closest('.z-password-toggle');
  return b && (togglePassword(b), true);
});

document.addEventListener('input', (e) => {
  const t = e.target;
  if (!(t instanceof HTMLInputElement)) return;
  if (t.matches('.z-mask-input')) maskInput(t, e);
  else if (t.matches('[data-zeyn-password] input')) meterInput(t);
});

document.addEventListener('focusin', (e) => {
  const t = e.target;
  if (t instanceof Element && t.matches('.z-mask > .z-native')) t.parentElement.querySelector('.z-mask-input')?.focus();
});

document.addEventListener(
  'submit',
  (e) => {
    for (const b of e.target.querySelectorAll?.('.z-password-toggle[aria-pressed="true"]') ?? []) togglePassword(b);
  },
  true,
);

/** Validation marks set on the raw input show on the masked one. */
new MutationObserver((records) => {
  for (const r of records) {
    const el = r.target;
    if (!(el instanceof Element) || !el.matches('.z-mask > .z-native')) continue;
    const shown = el.parentElement.querySelector('.z-mask-input');
    if (el.getAttribute('aria-invalid') === 'true') shown?.setAttribute('aria-invalid', 'true');
    else shown?.removeAttribute('aria-invalid');
  }
}).observe(document.body, { subtree: true, attributes: true, attributeFilter: ['aria-invalid'] });

initMasks(document);
initPasswords(document);

/* Multi select ------------------------------------------------------------------------------- */

/** Turns `[data-zeyn-multi]` multiple selects into chips and a combobox with a multi select list. */
function initMultis(root) {
  root.querySelectorAll('[data-zeyn-multi]:not([data-ready])').forEach((wrap) => {
    const native = wrap.querySelector('select');
    if (!native) return;
    wrap.dataset.ready = '';
    const t = texts(wrap);
    const box = make('div', 'z-lookup-box z-multi-box');
    const tags = make('ul', 'z-tags', { 'aria-label': t.selected });
    const combo = make('div', 'z-multi-button', {
      role: 'combobox',
      tabindex: native.disabled ? '-1' : '0',
      'aria-haspopup': 'listbox',
      'aria-expanded': 'false',
      'aria-labelledby': `${native.id}-label ${native.id}-value`,
      'data-testid': `${native.dataset.testid ?? ''}.combobox`,
    });
    for (const a of ['aria-describedby', 'aria-required', 'aria-invalid']) if (native.hasAttribute(a)) combo.setAttribute(a, native.getAttribute(a));
    if (native.required) combo.setAttribute('aria-required', 'true');
    combo.append(make('span', 'z-select-value', { id: `${native.id}-value` }));
    box.append(tags, combo, make('span', 'z-select-arrow', { 'aria-hidden': 'true' }));
    wrap.prepend(box);
    wrap.append(make('span', 'z-sr-only', { 'aria-live': 'polite' }));
    wrap.dataset.zeynLive = '';
    hideNative(native);
    syncMulti(wrap);
  });
}

/** Draws the chosen options of a multi select as chips and a count. */
function syncMulti(wrap) {
  const t = texts(wrap);
  const native = wrap.querySelector('select');
  const chosen = [...native.selectedOptions];
  wrap.querySelector('.z-tags').replaceChildren(
    ...chosen.map((o) => {
      const li = make('li', 'z-tag');
      li.append(make('span', '', { text: o.text }));
      const remove = make('button', 'z-tag-remove', { type: 'button', 'data-multi-remove': o.value, 'aria-label': fill(t.remove, { label: o.text }) });
      remove.append(make('span', '', { 'aria-hidden': 'true', text: '×' }));
      li.append(remove);
      return li;
    }),
  );
  const value = wrap.querySelector('.z-select-value');
  value.textContent = chosen.length ? fill(t.count, { n: chosen.length }) : t.none;
  value.classList.toggle('z-placeholder', !chosen.length);
}

/** Selects or clears one option of a multi select and tells the form. */
function setMulti(wrap, value, on) {
  const native = wrap.querySelector('select');
  const option = [...native.options].find((o) => o.value === value);
  if (!option || option.selected === on) return;
  option.selected = on;
  native.dispatchEvent(new Event('input', { bubbles: true }));
  native.dispatchEvent(new Event('change', { bubbles: true }));
  syncMulti(wrap);
  const item = lb?.items.find((i) => i.value === value);
  if (item) {
    item.selected = on;
    item.el.setAttribute('aria-selected', String(on));
  }
  announce(wrap, fill(texts(wrap).count, { n: native.selectedOptions.length }));
}

/** Opens the list of a multi select; options stay selected as the user toggles them. */
function openMulti(combo, where = 'first') {
  const wrap = combo.closest('[data-zeyn-multi]');
  const native = wrap.querySelector('select');
  openListbox(combo, {
    label: document.getElementById(`${native.id}-label`)?.textContent.replace('*', '').trim() ?? '',
    multiple: true,
    anchor: wrap.querySelector('.z-multi-box'),
    pick: (item) => setMulti(wrap, item.value, !item.selected),
  });
  const items = [...native.options].map((o) => ({ value: o.value, label: o.text, selected: o.selected }));
  fillListbox(items, native.selectedOptions[0]?.value);
  if (where === 'last') setActive(items.length - 1);
  if (where === 'first' && !native.selectedOptions.length) setActive(0);
}

/**
 * Keyboard of a multi select (APG listbox with multiple selection, inside a combobox): arrows,
 * Home and End move, Space and Enter toggle, Shift with an arrow moves and toggles, letters jump,
 * Backspace on the closed combobox removes the last chip. Ctrl+A is the browser's and stays so.
 */
function multiKey(e, combo) {
  const wrap = combo.closest('[data-zeyn-multi]');
  const open = lb?.combo === combo;
  if (!open) {
    if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(e.key)) {
      e.preventDefault();
      openMulti(combo);
      return true;
    }
    if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      openMulti(combo, e.key === 'Home' ? 'first' : 'last');
      return true;
    }
    if (e.key === 'Backspace' || e.key === 'Delete') {
      const last = [...wrap.querySelector('select').selectedOptions].at(-1);
      if (!last) return false;
      e.preventDefault();
      setMulti(wrap, last.value, false);
      return true;
    }
    const items = [...wrap.querySelector('select').options].map((o) => ({ label: o.text }));
    const at = typeAhead(e, items, 0);
    if (at < 0) return false;
    e.preventDefault();
    openMulti(combo);
    setActive(at);
    return true;
  }
  if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault();
    return pickActive();
  }
  if (e.altKey && e.key === 'ArrowUp') {
    e.preventDefault();
    closePopup(true);
    return true;
  }
  if (e.key === 'Tab') {
    closePopup(false);
    return false;
  }
  if (e.shiftKey && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
    moveActive(e);
    return pickActive();
  }
  if (moveActive(e)) return true;
  const at = typeAhead(e, lb.items, Math.max(0, lb.active));
  if (at < 0) return false;
  e.preventDefault();
  setActive(at);
  return true;
}

/* Lookup columns ----------------------------------------------------------------------------- */

/** Gives the open lookup list the column headers of a multi column lookup (`data-columns`). */
function lookupColumns(wrap) {
  const columns = parse(wrap.dataset.columns ?? 'null');
  if (!lb || !Array.isArray(columns) || !columns.length) return;
  lb.columns = columns;
  lb.panel.classList.add('z-list-columns');
  lb.panel.style.setProperty('--z-cols', String(columns.length));
  const head = make('div', 'z-list-head', { 'aria-hidden': 'true' });
  columns.forEach((c) => head.append(make('span', '', { text: c })));
  lb.panel.prepend(head);
}

/* Setup of the multi select ------------------------------------------------------------------ */

initializers.push(initMultis);

keyHandlers.push((e, t) => t.matches('.z-multi-button') && multiKey(e, t));

clickHandlers.push((e, t) => {
  const remove = t.closest('[data-multi-remove]');
  if (remove) {
    const wrap = remove.closest('[data-zeyn-multi]');
    setMulti(wrap, remove.dataset.multiRemove, false);
    wrap.querySelector('[role="combobox"]').focus();
    return true;
  }
  const box = t.closest('.z-multi-box');
  if (!box) return false;
  const combo = box.querySelector('[role="combobox"]');
  if (lb?.combo === combo) closePopup(true);
  else {
    combo.focus();
    openMulti(combo);
  }
  return true;
});

document.addEventListener('focusin', (e) => {
  const t = e.target;
  if (t instanceof Element && t.matches('[data-zeyn-multi] .z-native')) t.closest('[data-zeyn-multi]').querySelector('[role="combobox"]')?.focus();
});

/** Validation marks set on the native multiple select show on its combobox. */
new MutationObserver((records) => {
  for (const r of records) {
    const el = r.target;
    if (!(el instanceof Element) || !el.matches('[data-zeyn-multi] .z-native')) continue;
    const combo = el.closest('[data-zeyn-multi]').querySelector('[role="combobox"]');
    if (el.getAttribute('aria-invalid') === 'true') combo?.setAttribute('aria-invalid', 'true');
    else combo?.removeAttribute('aria-invalid');
  }
}).observe(document.body, { subtree: true, attributes: true, attributeFilter: ['aria-invalid'] });

initMultis(document);

/* Tooltip ------------------------------------------------------------------------------------ */

const TIP_DELAY_MS = 300;
const LONG_PRESS_MS = 500;

/** The shown tooltip `{ trigger, tip }`, its show and hide timers, and a long press in progress. */
let tipShown = null;
let tipTimer = 0;
let tipHideTimer = 0;
let pressTimer = 0;
let pressedTip = null;
let tipWasShown = false;

/** Places a tooltip above its trigger, or below when there is no room, inside the viewport. */
function placeTip(trigger, tip) {
  const a = trigger.getBoundingClientRect();
  const w = tip.offsetWidth;
  const h = tip.offsetHeight;
  let top = a.top - h - 6;
  if (top < 4) top = a.bottom + 6;
  const left = Math.max(4, Math.min(a.left + a.width / 2 - w / 2, innerWidth - w - 4));
  tip.style.top = `${Math.round(top)}px`;
  tip.style.left = `${Math.round(left)}px`;
}

/** Shows the tooltip of a trigger (`data-zeyn-tip` names it) and hides any other. */
function showTip(trigger) {
  clearTimeout(tipTimer);
  clearTimeout(tipHideTimer);
  const tip = document.getElementById(trigger.dataset.zeynTip);
  if (!tip) return;
  if (tipShown?.tip !== tip) hideTip();
  tip.hidden = false;
  tipShown = { trigger, tip };
  placeTip(trigger, tip);
}

/** Hides the shown tooltip. */
function hideTip() {
  clearTimeout(tipTimer);
  clearTimeout(tipHideTimer);
  if (!tipShown) return;
  tipShown.tip.hidden = true;
  tipShown = null;
}

/** Hides the tooltip soon, unless the pointer moves onto it or back to its trigger (WCAG 1.4.13). */
function hideTipSoon() {
  clearTimeout(tipTimer);
  clearTimeout(tipHideTimer);
  tipHideTimer = setTimeout(hideTip, 150);
}

document.addEventListener('pointerover', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t || e.pointerType === 'touch') return;
  if (tipShown && (tipShown.tip.contains(t) || tipShown.trigger.contains(t))) return clearTimeout(tipHideTimer);
  const trigger = t.closest('[data-zeyn-tip]');
  if (!trigger) return;
  clearTimeout(tipTimer);
  tipTimer = setTimeout(() => showTip(trigger), tipShown ? 0 : TIP_DELAY_MS);
});

document.addEventListener('pointerout', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t || e.pointerType === 'touch') return;
  if (t.closest('[data-zeyn-tip]') || (tipShown && tipShown.tip.contains(t))) {
    clearTimeout(tipTimer);
    if (tipShown && document.activeElement !== tipShown.trigger) hideTipSoon();
  }
});

document.addEventListener('focusin', (e) => {
  const trigger = e.target instanceof Element ? e.target.closest('[data-zeyn-tip]') : null;
  if (trigger) showTip(trigger);
  else if (tipShown && !tipShown.tip.contains(e.target)) hideTip();
});

document.addEventListener('focusout', (e) => {
  if (tipShown && e.target === tipShown.trigger) hideTip();
});

document.addEventListener(
  'keydown',
  (e) => {
    if (e.key !== 'Escape' || !tipShown) return;
    e.preventDefault();
    e.stopPropagation();
    hideTip();
  },
  true,
);

document.addEventListener(
  'pointerdown',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    if (tipShown && t && !tipShown.tip.contains(t) && !tipShown.trigger.contains(t)) hideTip();
    const trigger = t?.closest('[data-zeyn-tip]');
    tipWasShown = !!trigger && tipShown?.trigger === trigger;
    clearTimeout(pressTimer);
    pressedTip = null;
    if (!trigger || e.pointerType !== 'touch') return;
    pressTimer = setTimeout(() => {
      pressedTip = trigger;
      showTip(trigger);
    }, LONG_PRESS_MS);
  },
  true,
);

for (const type of ['pointerup', 'pointercancel']) document.addEventListener(type, () => clearTimeout(pressTimer), true);

/** A long press shows the tooltip and does not also press the button; a tap on an info button toggles it. */
document.addEventListener(
  'click',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    const trigger = t?.closest('[data-zeyn-tip]');
    if (pressedTip && trigger === pressedTip) {
      pressedTip = null;
      e.preventDefault();
      e.stopPropagation();
      return;
    }
    if (!trigger?.classList.contains('z-tip-button')) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.detail > 0 ? tipWasShown : tipShown?.trigger === trigger) hideTip();
    else showTip(trigger);
  },
  true,
);

document.addEventListener('contextmenu', (e) => {
  if (pressedTip && e.target instanceof Element && e.target.closest('[data-zeyn-tip]') === pressedTip) e.preventDefault();
});

/* Popover ------------------------------------------------------------------------------------ */

/** The open popover `{ button, pop }`. */
let pop = null;

/** Places an open popover below its button, or above it when there is no room below. */
function placePopover() {
  if (!pop) return;
  const { button, pop: el } = pop;
  if (NARROW.matches) {
    el.style.removeProperty('top');
    el.style.removeProperty('left');
    return;
  }
  const a = button.getBoundingClientRect();
  const w = el.offsetWidth;
  const h = el.offsetHeight;
  const below = innerHeight - a.bottom - 8;
  const top = below < h && a.top - 8 > below ? Math.max(8, a.top - h - 4) : Math.min(a.bottom + 4, Math.max(8, innerHeight - h - 8));
  const rtl = getComputedStyle(button).direction === 'rtl';
  const left = Math.max(8, Math.min(rtl ? a.right - w : a.left, innerWidth - w - 8));
  el.style.top = `${Math.round(top)}px`;
  el.style.left = `${Math.round(left)}px`;
}

/** Opens a popover and moves focus to its first control, or to the dialog itself. */
function openPopover(button) {
  closePopover(false);
  const el = document.getElementById(button.getAttribute('aria-controls'));
  if (!el) return;
  el.hidden = false;
  button.setAttribute('aria-expanded', 'true');
  pop = { button, pop: el };
  placePopover();
  const first = focusables(el).find((x) => !x.matches('[data-zeyn-popover-close]'));
  (first ?? el).focus();
}

/** Closes the open popover; focus goes back to its button unless it already moved elsewhere. */
function closePopover(returnFocus = true) {
  if (!pop) return;
  const { button, pop: el } = pop;
  pop = null;
  el.hidden = true;
  button.setAttribute('aria-expanded', 'false');
  if (returnFocus) button.focus();
}

clickHandlers.push((e, t) => {
  const button = t.closest('.z-popover-button');
  if (button) {
    if (pop?.button === button) closePopover(true);
    else openPopover(button);
    return true;
  }
  if (t.closest('[data-zeyn-popover-close]')) {
    closePopover(true);
    return true;
  }
  return false;
});

keyHandlers.unshift((e, t) => {
  if (e.key !== 'Escape' || !pop || popup || !(pop.pop.contains(t) || pop.button === t)) return false;
  e.preventDefault();
  closePopover(true);
  return true;
});

document.addEventListener(
  'pointerdown',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    if (!pop || !t || pop.pop.contains(t) || pop.button.contains(t) || popup?.panel.contains(t)) return;
    closePopover(false);
  },
  true,
);

document.addEventListener('focusout', (e) => {
  if (!pop || !pop.pop.contains(e.target)) return;
  setTimeout(() => {
    const a = document.activeElement;
    if (pop && a && a !== document.body && !pop.pop.contains(a) && a !== pop.button && !popup?.panel.contains(a)) closePopover(false);
  });
});

window.addEventListener('resize', placePopover);
document.addEventListener('scroll', (e) => pop && !pop.pop.contains(e.target) && placePopover(), true);
document.addEventListener('zeyn:leave', () => {
  closePopover(false);
  hideTip();
});

/* Busy overlay ------------------------------------------------------------------------------- */

const BUSY_MIN_MS = 300;

/** State of each busy container: `{ on, timer, shownAt }`. */
const busyStates = new WeakMap();

/**
 * Marks a `[data-zeyn-busy]` container busy while anything inside it is `aria-busy`, and shows
 * its overlay after the container's delay; once shown it stays at least `BUSY_MIN_MS`.
 */
function updateBusy(box) {
  const s = busyStates.get(box) ?? { on: false, timer: 0, shownAt: 0 };
  busyStates.set(box, s);
  const busy = [...box.querySelectorAll('[aria-busy="true"]')].some((el) => el.closest('[data-zeyn-busy]') === box && el !== box);
  const overlay = box.querySelector(':scope > .z-busy-overlay');
  const text = overlay?.querySelector('.z-busy-text');
  if (busy && !s.on) {
    s.on = true;
    box.setAttribute('aria-busy', 'true');
    clearTimeout(s.timer);
    s.timer = setTimeout(() => {
      if (!overlay) return;
      overlay.hidden = false;
      if (text) text.textContent = text.dataset.text ?? '';
      s.shownAt = Date.now();
    }, Number(box.dataset.zeynBusy) || 400);
  } else if (!busy && s.on) {
    s.on = false;
    box.removeAttribute('aria-busy');
    clearTimeout(s.timer);
    const hide = () => {
      if (s.on || !overlay) return;
      overlay.hidden = true;
      if (text) text.textContent = '';
    };
    if (overlay && !overlay.hidden) s.timer = setTimeout(hide, Math.max(0, BUSY_MIN_MS - (Date.now() - s.shownAt)));
  }
}

new MutationObserver((records) => {
  for (const r of records) {
    const el = r.target;
    if (!(el instanceof Element) || (r.type === 'attributes' && el.matches('[data-zeyn-busy]'))) continue;
    const box = el.closest('[data-zeyn-busy]');
    if (box) updateBusy(box);
  }
}).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['aria-busy'] });

/* Menu button and split button --------------------------------------------------------------- */

/** The open command menu `{ button, menu }`. */
let cmdMenu = null;
/** The button of the menu whose command ran last; focus goes back to it after a confirm dialog. */
let menuOpener = null;

/** Commands of a menu that can be chosen, in order. */
function menuEntries(menu) {
  return [...menu.querySelectorAll('[role="menuitem"]:not([disabled])')];
}

/** Places an open menu below its button, or above when there is no room, inside the viewport. */
function placeMenu() {
  if (!cmdMenu) return;
  const { button, menu } = cmdMenu;
  const a = button.getBoundingClientRect();
  const w = menu.offsetWidth;
  const h = menu.offsetHeight;
  const below = innerHeight - a.bottom - 8;
  const top = below < h && a.top - 8 > below ? Math.max(8, a.top - h - 4) : Math.min(a.bottom + 4, Math.max(8, innerHeight - h - 8));
  const box = button.closest('.z-split-button') ?? button;
  const b = box.getBoundingClientRect();
  const rtl = getComputedStyle(button).direction === 'rtl';
  const left = Math.max(8, Math.min(rtl ? b.right - w : b.left, innerWidth - w - 8));
  menu.style.top = `${Math.round(top)}px`;
  menu.style.left = `${Math.round(left)}px`;
}

/** Opens the menu of a menu button and focuses its first or last command. */
function openCmdMenu(button, where = 'first') {
  closeCmdMenu(false);
  const menu = document.getElementById(button.getAttribute('aria-controls'));
  if (!menu) return;
  menu.hidden = false;
  button.setAttribute('aria-expanded', 'true');
  cmdMenu = { button, menu };
  placeMenu();
  const entries = menuEntries(menu);
  (where === 'last' ? entries.at(-1) : entries[0])?.focus();
}

/** Closes the open menu; focus goes back to its button when asked. */
function closeCmdMenu(returnFocus = true) {
  if (!cmdMenu) return;
  const { button, menu } = cmdMenu;
  cmdMenu = null;
  menu.hidden = true;
  button.setAttribute('aria-expanded', 'false');
  if (returnFocus) button.focus();
}

/** Keys of a menu button and of its open menu (APG menu button and menu patterns). */
function cmdMenuKey(e, t) {
  if (t.matches('.z-menu-trigger')) {
    const where = { ArrowDown: 'first', Enter: 'first', ' ': 'first', ArrowUp: 'last' }[e.key];
    if (!where) return false;
    e.preventDefault();
    openCmdMenu(t, where);
    return true;
  }
  if (!cmdMenu || !t.matches('[role="menuitem"]') || !cmdMenu.menu.contains(t)) return false;
  const entries = menuEntries(cmdMenu.menu);
  const i = entries.indexOf(t);
  const to = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: entries.length - 1 }[e.key];
  if (to !== undefined) {
    e.preventDefault();
    entries[(to + entries.length) % entries.length]?.focus();
    return true;
  }
  if (e.key === 'Escape') {
    e.preventDefault();
    closeCmdMenu(true);
    return true;
  }
  if (e.key === 'Tab') {
    closeCmdMenu(false);
    cmdMenu = null;
    return false;
  }
  const at = typeAhead(e, entries.map((x) => ({ label: x.textContent })), Math.max(0, i));
  if (at < 0) return false;
  e.preventDefault();
  entries[at].focus();
  return true;
}

keyHandlers.unshift((e, t) => cmdMenuKey(e, t));

clickHandlers.unshift((e, t) => {
  const trigger = t.closest('.z-menu-trigger');
  if (trigger) {
    if (cmdMenu?.button === trigger) closeCmdMenu(true);
    else openCmdMenu(trigger);
    return true;
  }
  const entry = t.closest('.z-menu-list [role="menuitem"]');
  if (entry && cmdMenu?.menu.contains(entry)) {
    const button = cmdMenu.button;
    menuOpener = button;
    setTimeout(() => {
      closeCmdMenu(false);
      if (!document.activeElement || document.activeElement === document.body || cmdMenu?.menu.contains(document.activeElement)) button.focus();
    });
  }
  return false;
});

document.addEventListener(
  'pointerdown',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    if (cmdMenu && t && !cmdMenu.menu.contains(t) && !cmdMenu.button.contains(t)) closeCmdMenu(false);
  },
  true,
);

document.addEventListener('focusout', (e) => {
  if (!cmdMenu || !cmdMenu.menu.contains(e.target)) return;
  setTimeout(() => {
    const a = document.activeElement;
    if (cmdMenu && a && a !== document.body && !cmdMenu.menu.contains(a) && a !== cmdMenu.button && !a.closest('dialog.z-confirm')) closeCmdMenu(false);
  });
});

window.addEventListener('resize', placeMenu);
document.addEventListener('scroll', (e) => cmdMenu && !cmdMenu.menu.contains(e.target) && placeMenu(), true);
document.addEventListener('zeyn:leave', () => closeCmdMenu(false));

document.addEventListener(
  'close',
  () => {
    setTimeout(() => {
      if (menuOpener?.isConnected && (!document.activeElement || document.activeElement === document.body)) menuOpener.focus();
      menuOpener = null;
    });
  },
  true,
);
