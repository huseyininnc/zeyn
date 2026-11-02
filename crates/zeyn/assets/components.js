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
const LOOKUP_DELAY_MS = 250;

/** Returns the CSRF token from `meta[name=zeyn-csrf]`. */
function csrf() {
  return document.querySelector('meta[name="zeyn-csrf"]')?.content ?? '';
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
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': csrf() },
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
  const region = el.closest('[data-zeyn-live]')?.querySelector('[aria-live]') ?? document.querySelector('.z-toasts');
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

/** Marks the chosen hour, minute and second, keeps them in view and sets the roving focus. */
function drawClock(moveFocus) {
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
    active.scrollIntoView({ block: 'center', behavior: 'instant' });
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
  drawClock(false);
  list.focus();
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

/* Setup -------------------------------------------------------------------------------------- */

/** Initializers run for every root that gets new content. */
const initializers = [restoreTabs];

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
  (e, t) => t.matches('[role="tab"]') && tabKey(e, t),
  (e, t) => t.matches('.z-split-bar') && splitKey(e, t),
];

/** Click handlers, tried in order until one handles the event. */
const clickHandlers = [
  pickerClick,
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

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));

new MutationObserver((records) => {
  for (const r of records) for (const n of r.addedNodes) if (n instanceof HTMLDialogElement) init(n);
}).observe(document.body, { childList: true });

init(document);
