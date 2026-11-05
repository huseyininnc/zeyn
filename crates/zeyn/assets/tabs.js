/**
 * Collapsible panels, tabs (with lazy panels and remembered choices) and split panes.
 * Loaded only by pages that hold one (`data-zeyn-toggle`, `data-zeyn-tabs`, `data-zeyn-split`).
 * @module tabs
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { BASE, VERSION, address, clickHandlers, initializers, keyHandlers, make, parse, post, readingKey } = K;

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
  if (remember && root?.dataset.zeynRemember) rememberTab(root, tab);
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
  const next = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: tabs.length - 1 }[readingKey(e.key, tab)];
  if (next === undefined) return false;
  e.preventDefault();
  selectTab(tabs[(next + tabs.length) % tabs.length], { focus: true });
  return true;
}

/** The client preference store (`state.js`), imported by the first tabs bound to a key. */
function prefStore() {
  return import(`${BASE}/_zeyn/state.js${VERSION}`);
}

/** Keeps the tab a user opened under the key of tabs with `Tabs::remember`. */
function rememberTab(root, tab) {
  prefStore().then((m) => {
    const key = m.keyOf(root);
    if (key) m.set(key, tab.dataset.zeynTab);
  });
}

/** Opens the tabs named in the URL hash, else the remembered tab of tabs bound to a key. */
function restoreTabs(root) {
  const state = hashState();
  const find = (tabs, wanted) => wanted && [...tabs.querySelectorAll('[role="tab"]')].find((t) => t.dataset.zeynTab === wanted);
  root.querySelectorAll('[data-zeyn-tabs]').forEach((tabs) => {
    const tab = find(tabs, state.get(tabs.dataset.zeynTabs));
    if (tab && tab.getAttribute('aria-selected') !== 'true') selectTab(tab, { remember: false });
    if (tab || !tabs.dataset.zeynRemember) return;
    prefStore().then((m) => {
      const key = m.keyOf(tabs);
      const kept = key && find(tabs, m.get(key));
      if (kept && kept.getAttribute('aria-selected') !== 'true') selectTab(kept, { remember: false });
    });
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
    const across = getComputedStyle(split).direction === 'rtl' ? box.right - ev.clientX : ev.clientX - box.left;
    const percent = vertical ? ((ev.clientY - box.top) / box.height) * 100 : (across / box.width) * 100;
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
  const step = { ArrowLeft: -5, ArrowUp: -5, ArrowRight: 5, ArrowDown: 5 }[readingKey(e.key, bar)];
  if (step !== undefined) setSplit(bar, now + step);
  else if (e.key === 'Home') setSplit(bar, 10);
  else if (e.key === 'End') setSplit(bar, 90);
  else return false;
  e.preventDefault();
  return true;
}

/* Setup of panels, tabs and split panes ------------------------------------------------------ */

initializers.push(restoreTabs);

keyHandlers.push(
  (e, t) => t.matches('[role="tab"]') && tabKey(e, t),
  (e, t) => t.matches('.z-split-bar') && splitKey(e, t),
);

clickHandlers.push(
  (e, t) => {
    const b = t.closest('[data-zeyn-toggle]');
    return b && (togglePanel(b), true);
  },
  (e, t) => {
    const tab = t.closest('[data-zeyn-tabs] [role="tab"]');
    return tab && (selectTab(tab), true);
  },
);

document.addEventListener(
  'pointerdown',
  (e) => {
    const bar = e.target instanceof Element ? e.target.closest('.z-split-bar') : null;
    if (bar && e.button === 0) dragSplit(e, bar);
  },
  true,
);

window.addEventListener('hashchange', () => restoreTabs(document));

restoreTabs(document);
