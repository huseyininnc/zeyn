/**
 * Zeyn component library runtime: the shared part.
 *
 * Helpers, the popup and listbox cores, selects (the pickers of the app shell are selects),
 * confirm dialogs, dismissible alerts and breadcrumbs, and the delegation every component module
 * uses: `initializers`, `keyHandlers` and `clickHandlers`, run from listeners on `document`. Every
 * other component has its own module (`tabs.js`, `dates.js`, `choices.js`, `inputs.js`,
 * `lines.js`, `steps.js`, `tree.js`, `overlays.js`, `menubutton.js`) that a page loads only when
 * it holds that component (`server/modules.rs`, section 1.1 of `docs/runtime-protocol.md`). A
 * module imports this one, adds its handlers and sets up the document when it loads.
 * Components are initialized on page load, on the `zeyn:content` event (nav.js and lazy panels)
 * and when a window `<dialog>` opens. Every server call goes through `/_zeyn/node` or
 * `/_zeyn/data`, which check access on the server.
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

/* Shared helpers ----------------------------------------------------------------------------- */

/** Page language; names of months and days follow it, not the browser's language. */
function lang() {
  return document.documentElement.lang || 'en';
}

/** Localized texts of a picker root, from `data-texts`. */
function texts(root) {
  root.zeynTexts ??= parse(root.dataset.texts ?? '{}') ?? {};
  return root.zeynTexts;
}

/**
 * The key as it reads at `el`: under `dir="rtl"` Left and Right swap, so ArrowRight still means
 * previous and ArrowLeft next on a page laid out right to left (W3C APG).
 * @example readingKey('ArrowLeft', tabOnAnArabicPage) === 'ArrowRight'
 */
function readingKey(key, el) {
  if (key !== 'ArrowLeft' && key !== 'ArrowRight') return key;
  if (getComputedStyle(el).direction !== 'rtl') return key;
  return key === 'ArrowLeft' ? 'ArrowRight' : 'ArrowLeft';
}


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

/** A number for element ids, unique on the page. */
function uid() {
  return ++listUid;
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
    wrap.zeynSync = () => syncSelect(wrap);
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
    label: document.getElementById(`${native.id}-label`)?.textContent.replace(/\*/g, '').trim() ?? '',
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

/**
 * Asks before submitting forms that carry `data-zeyn-confirm` (action buttons), or through a
 * submit button that carries it (workflow transitions).
 */
async function confirmSubmit(e) {
  const form = e.target;
  if (!(form instanceof HTMLFormElement)) return;
  const by = e.submitter instanceof HTMLElement && e.submitter.dataset.zeynConfirm ? e.submitter : null;
  const d = (by ?? form).dataset;
  if (!d.zeynConfirm) return;
  if (form.zeynConfirmed) {
    form.zeynConfirmed = false;
    return;
  }
  e.preventDefault();
  e.stopImmediatePropagation();
  const danger = by ? by.matches('.z-danger-button') : !!form.querySelector('.z-danger-button');
  if (await confirmDialog({ title: d.textTitle, message: d.zeynConfirm, ok: d.textOk, cancel: d.textCancel, danger })) {
    form.zeynConfirmed = true;
    form.requestSubmit(by ?? undefined);
  }
}

/** Prefixes app relative links (breadcrumbs) with the path the app is served under. */
function baseLinks(root) {
  root.querySelectorAll('a[data-zeyn-base-href]').forEach((a) => {
    a.setAttribute('href', BASE + a.getAttribute('href'));
    a.removeAttribute('data-zeyn-base-href');
  });
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

/** Initializers run for every root that gets new content; component modules add theirs. */
const initializers = [initSelects, baseLinks];

/** Initializes every component inside `root`. Safe to call more than once. */
function init(root) {
  for (const fn of initializers) fn(root);
}

/** Keydown handlers, tried in order until one handles the event; component modules add theirs. */
const keyHandlers = [(e) => popupKey(e), (e, t) => t.matches('.z-select-button') && selectKey(e, t)];

/** Click handlers, tried in order until one handles the event; component modules add theirs. */
const clickHandlers = [
  (e, t) => {
    const combo = t.closest('.z-select-button');
    if (!combo) return false;
    if (lb?.combo === combo) closePopup(true);
    else openSelect(combo);
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

document.addEventListener('focusin', (e) => {
  const t = e.target;
  if (t instanceof Element && t.classList.contains('z-native')) controlOf(t)?.focus();
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

document.addEventListener(
  'pointerdown',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    if (t && listboxPointer(e)) return;
    if (popup && t && !popup.panel.contains(t) && !popup.anchor.contains(t)) closePopup(false);
  },
  true,
);

window.addEventListener('resize', placePopup);
document.addEventListener('scroll', (e) => popup && !popup.panel.contains(e.target) && placePopup(), true);
NARROW.addEventListener('change', placePopup);

document.addEventListener('zeyn:leave', () => closePopup(false));
document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));

new MutationObserver((records) => {
  for (const r of records) for (const n of r.addedNodes) if (n instanceof HTMLDialogElement) init(n);
}).observe(document.body, { childList: true });

init(document);
export { BASE, LOOKUP_DELAY_MS, NARROW, VERSION, address, announce, clickHandlers, closePopup, fetchData, fill, fillListbox, focusables, fold, hideNative, initializers, keyHandlers, lang, lb, make, moveActive, openListbox, openPopup, parse, pickActive, placePopup, popup, post, readingKey, reveal, setActive, texts, typeAhead, uid };
