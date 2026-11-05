/**
 * Zeyn desktop class shell.
 *
 * Menu bars, toolboxes, status bars, docking panels and the folded sections of property
 * inspectors. Loaded only by pages that hold one of them (`data-zeyn-menubar`,
 * `data-zeyn-toolbox`, `data-zeyn-status-live`, `data-zeyn-dock`, `.z-form-section`).
 * Interaction is wired with event delegation on `document`.
 * @module pro
 */
const B = await import(new URL(`palette.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { fill, fold, load, make, post, run, runCommand, store } = B;

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
  const rtl = getComputedStyle(box).direction === 'rtl';
  const step = { ArrowDown: 1, ArrowRight: rtl ? -1 : 1, ArrowUp: -1, ArrowLeft: rtl ? 1 : -1 }[e.key];
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

/* Small popup menus (dock panel options) ----------------------------------------------------- */

/** The open popup menu: `{menu, opener}`. */
let popMenu = null;

/**
 * Opens a menu under `opener`. Items are `{label, run, checked?}` or `{separator: true}`.
 * Keyboard as in the menu bar: arrows, Home, End, Enter, Space, Escape and Tab.
 */
function openPopMenu(opener, items) {
  closePopMenu(false);
  const menu = make('ul', 'z-menu z-pop-menu', { role: 'menu', 'aria-label': opener.getAttribute('aria-label') ?? '' });
  for (const item of items) {
    if (item.separator) {
      if (menu.lastChild && !menu.lastChild.matches('[role="separator"]')) menu.append(make('li', 'z-menu-sep', { role: 'separator' }));
      continue;
    }
    const li = make('li', '', { role: 'none' });
    const role = item.checked == null ? 'menuitem' : 'menuitemcheckbox';
    const b = make('button', 'z-menu-item', { type: 'button', role, tabindex: '-1' });
    if (item.checked != null) b.setAttribute('aria-checked', String(!!item.checked));
    b.append(make('span', 'z-menu-label', { text: item.label }));
    b.zeynRun = item.run;
    li.append(b);
    menu.append(li);
  }
  if (menu.lastChild?.matches('[role="separator"]')) menu.lastChild.remove();
  opener.after(menu);
  opener.setAttribute('aria-expanded', 'true');
  popMenu = { menu, opener };
  menu.querySelector('[role^="menuitem"]')?.focus();
}

function closePopMenu(focus = true) {
  const p = popMenu;
  if (!p) return;
  popMenu = null;
  p.menu.remove();
  p.opener.setAttribute('aria-expanded', 'false');
  if (focus && p.opener.isConnected) p.opener.focus();
}

function runPopItem(b) {
  const fn = b.zeynRun;
  closePopMenu(true);
  fn?.();
}

/** Keys in a popup menu. Returns true when handled. */
function popKey(e, t) {
  if (!popMenu?.menu.contains(t)) return false;
  const items = [...popMenu.menu.querySelectorAll('[role^="menuitem"]')];
  const i = items.indexOf(t);
  if (e.key === 'ArrowDown') items[(i + 1) % items.length]?.focus();
  else if (e.key === 'ArrowUp') items[(i - 1 + items.length) % items.length]?.focus();
  else if (e.key === 'Home') items[0]?.focus();
  else if (e.key === 'End') items.at(-1)?.focus();
  else if (e.key === 'Escape') closePopMenu(true);
  else if (e.key === 'Enter' || e.key === ' ') runPopItem(t);
  else if (e.key === 'Tab') {
    closePopMenu(false);
    return false;
  } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) typeAhead(items, t, e.key)?.focus();
  else return false;
  e.preventDefault();
  e.stopPropagation();
  return true;
}

/* Docking panels ----------------------------------------------------------------------------- */

const AREAS = ['left', 'center', 'bottom', 'right'];
const DOCK_MIN = 10;
const DOCK_MAX = 60;
const DOCK_SAVE_MS = 400;
const dockTimers = new WeakMap();

/** Screen, params and node of a dock, as `/_zeyn/dock` expects them. */
function dockAddress(dock) {
  const screen = dock.closest('[data-zeyn-screen]');
  let params = null;
  try {
    params = JSON.parse(screen?.dataset.zeynParams ?? 'null');
  } catch {
    params = null;
  }
  return { screen: screen?.dataset.zeynScreen ?? '', params, node: dock.closest('[data-zeyn-node]')?.dataset.zeynNode ?? '0' };
}

/** Browser storage key of a dock layout, when the server keeps none. */
function dockStoreKey(dock) {
  const a = dockAddress(dock);
  return `zeyn.dock.${location.pathname}.${a.screen}#${a.node}`;
}

function areaEl(dock, area) {
  return dock.querySelector(`:scope > .z-dock-area[data-area="${area}"]`);
}

/** Tabs of one area, or of the whole dock, in order. */
function dockTabs(dock, area) {
  const scope = area ? `:scope > .z-dock-area[data-area="${area}"]` : ':scope > .z-dock-area';
  return [...dock.querySelectorAll(`${scope} > .z-dock-head > .z-dock-tabs > [role="tab"]`)];
}

function dockPanel(tab) {
  return document.getElementById(tab.getAttribute('aria-controls'));
}

function dockTab(dock, id) {
  return dockTabs(dock).find((t) => t.dataset.panel === id) ?? null;
}

function areaOfTab(tab) {
  return tab.closest('.z-dock-area').dataset.area;
}

function dockTitle(tab) {
  return tab.textContent.trim();
}

/** Announces a change in the dock's live region. */
function dockSay(dock, key, values) {
  const live = dock.querySelector(':scope > [data-zeyn-dock-live]');
  if (live) live.textContent = fill(dock.dataset[`text${key[0].toUpperCase()}${key.slice(1)}`], values);
}

/** Area name as users read it. */
function areaName(dock, area) {
  return dock.dataset[`text${area[0].toUpperCase()}${area.slice(1)}`] ?? area;
}

/** Shows `tab`'s panel in its area and makes it the area's tab stop. */
function selectDockTab(tab, focus = false) {
  for (const t of dockTabs(tab.closest('.z-dock'), areaOfTab(tab))) {
    const on = t === tab;
    t.setAttribute('aria-selected', String(on));
    t.tabIndex = on ? 0 : -1;
    dockPanel(t).hidden = !on;
  }
  if (focus) tab.focus();
}

/**
 * Hides side areas without open panels, makes sure each area shows one open panel and marks an
 * empty center.
 */
function refreshDock(dock) {
  for (const area of AREAS) {
    const el = areaEl(dock, area);
    const open = dockTabs(dock, area).filter((t) => !t.hidden);
    if (area === 'center') {
      if (open.length) el.removeAttribute('data-empty');
      else el.dataset.empty = dock.dataset.textEmpty ?? '';
    } else el.hidden = !open.length;
    const bar = dock.querySelector(`:scope > .z-dock-bar[data-edge="${area}"]`);
    if (bar) bar.hidden = !open.length;
    dock.style.setProperty(`--z-dock-${area}-on`, open.length ? '1' : '0');
    const current = open.find((t) => t.getAttribute('aria-selected') === 'true');
    for (const t of dockTabs(dock, area)) if (t.hidden) dockPanel(t).hidden = true;
    if (open.length) selectDockTab(current ?? open[0]);
  }
}

function sizeOf(dock, edge) {
  return Number.parseInt(dock.style.getPropertyValue(`--z-dock-${edge}`), 10) || 20;
}

/** Sets the share of a side area, kept within the limits. */
function setDockSize(dock, edge, percent) {
  const v = Math.round(Math.min(DOCK_MAX, Math.max(DOCK_MIN, percent)));
  dock.style.setProperty(`--z-dock-${edge}`, `${v}%`);
  dock.querySelector(`:scope > .z-dock-bar[data-edge="${edge}"]`)?.setAttribute('aria-valuenow', String(v));
}

/** The layout as the server stores it. */
function dockLayout(dock) {
  const panels = [];
  for (const area of AREAS) {
    for (const t of dockTabs(dock, area)) {
      panels.push({ id: t.dataset.panel, area, open: !t.hidden, active: t.getAttribute('aria-selected') === 'true' && !t.hidden });
    }
  }
  return { sizes: { left: sizeOf(dock, 'left'), right: sizeOf(dock, 'right'), bottom: sizeOf(dock, 'bottom') }, panels };
}

/** Moves tabs and panels into a saved layout. Unknown panels are ignored. */
function applyDockLayout(dock, layout) {
  if (!layout || typeof layout !== 'object') return;
  for (const edge of ['left', 'right', 'bottom']) {
    const v = Number(layout.sizes?.[edge]);
    if (Number.isFinite(v)) setDockSize(dock, edge, v);
  }
  const active = [];
  for (const p of Array.isArray(layout.panels) ? layout.panels : []) {
    const tab = dockTab(dock, String(p?.id));
    if (!tab || !AREAS.includes(p.area)) continue;
    placeTab(dock, tab, p.area);
    tab.hidden = !p.open;
    if (p.active && p.open) active.push(tab);
  }
  for (const t of dockTabs(dock)) t.setAttribute('aria-selected', 'false');
  for (const t of active) t.setAttribute('aria-selected', 'true');
  refreshDock(dock);
}

/** Puts a tab and its panel at the end of an area. */
function placeTab(dock, tab, area) {
  const el = areaEl(dock, area);
  el.querySelector(':scope > .z-dock-head > .z-dock-tabs').append(tab);
  el.querySelector(':scope > .z-dock-body').append(dockPanel(tab));
}

/** Saves the layout after a short pause: on the server when it keeps layouts, else locally. */
function saveDock(dock) {
  clearTimeout(dockTimers.get(dock));
  dockTimers.set(
    dock,
    setTimeout(async () => {
      const layout = dockLayout(dock);
      if (dock.dataset.store === 'server') {
        const r = await post('/_zeyn/dock', { ...dockAddress(dock), save: true, layout });
        if (r.ok) return;
      }
      store(dockStoreKey(dock), layout);
    }, DOCK_SAVE_MS),
  );
}

/** Moves a panel to an area, opens and shows it. */
function moveDockPanel(dock, tab, area, focus = true) {
  placeTab(dock, tab, area);
  tab.hidden = false;
  selectDockTab(tab);
  refreshDock(dock);
  if (focus) tab.focus();
  dockSay(dock, 'moved', { panel: dockTitle(tab), area: areaName(dock, area) });
  saveDock(dock);
}

/** Closes a panel; focus goes to the next open tab of its area, or to the area's options. */
function closeDockPanel(dock, tab, focus = true) {
  const area = areaOfTab(tab);
  const open = dockTabs(dock, area).filter((t) => !t.hidden);
  const next = open[open.indexOf(tab) + 1] ?? open[open.indexOf(tab) - 1];
  tab.hidden = true;
  tab.setAttribute('aria-selected', 'false');
  dockPanel(tab).hidden = true;
  if (next) selectDockTab(next);
  refreshDock(dock);
  if (focus) (next ?? areaEl(dock, 'center').querySelector('.z-dock-options')).focus();
  dockSay(dock, 'closed', { panel: dockTitle(tab) });
  saveDock(dock);
}

/** Brings a closed panel back in the area it was in. */
function showDockPanel(dock, tab, focus = true) {
  tab.hidden = false;
  selectDockTab(tab);
  refreshDock(dock);
  if (focus) tab.focus();
  dockSay(dock, 'shown', { panel: dockTitle(tab) });
  saveDock(dock);
}

/** The options of an area: move or close its panel, show closed panels. */
function openDockOptions(button) {
  const dock = button.closest('.z-dock');
  const area = button.closest('.z-dock-area').dataset.area;
  const tab = dockTabs(dock, area).find((t) => !t.hidden && t.getAttribute('aria-selected') === 'true');
  const items = [];
  if (tab) {
    for (const to of AREAS) {
      if (to !== area) items.push({ label: fill(dock.dataset.textMove, { area: areaName(dock, to) }), run: () => moveDockPanel(dock, tab, to) });
    }
    items.push({ separator: true }, { label: fill(dock.dataset.textClose, { panel: dockTitle(tab) }), run: () => closeDockPanel(dock, tab) });
  }
  items.push({ separator: true });
  for (const t of dockTabs(dock).filter((x) => x.hidden)) {
    items.push({ label: fill(dock.dataset.textShow, { panel: dockTitle(t) }), run: () => showDockPanel(dock, t) });
  }
  if (items.some((i) => !i.separator)) openPopMenu(button, items);
}

/** Keys on a dock tab or divider. Returns true when handled. */
function dockKey(e, t) {
  const dock = t.closest('.z-dock');
  if (t.matches('.z-dock-bar')) {
    const edge = t.dataset.edge;
    const rtl = getComputedStyle(dock).direction === 'rtl';
    const grow = { left: rtl ? 'ArrowLeft' : 'ArrowRight', right: rtl ? 'ArrowRight' : 'ArrowLeft', bottom: 'ArrowUp' }[edge];
    const shrink = { left: rtl ? 'ArrowRight' : 'ArrowLeft', right: rtl ? 'ArrowLeft' : 'ArrowRight', bottom: 'ArrowDown' }[edge];
    const now = sizeOf(dock, edge);
    if (e.key === grow) setDockSize(dock, edge, now + 5);
    else if (e.key === shrink) setDockSize(dock, edge, now - 5);
    else if (e.key === 'Home') setDockSize(dock, edge, DOCK_MIN);
    else if (e.key === 'End') setDockSize(dock, edge, DOCK_MAX);
    else return false;
    e.preventDefault();
    saveDock(dock);
    return true;
  }
  if (!t.matches('.z-dock-tab')) return false;
  const tabs = dockTabs(dock, areaOfTab(t)).filter((x) => !x.hidden);
  const i = tabs.indexOf(t);
  const rtl = getComputedStyle(dock).direction === 'rtl';
  const next = rtl ? 'ArrowLeft' : 'ArrowRight';
  const prev = rtl ? 'ArrowRight' : 'ArrowLeft';
  let target = null;
  if (e.key === next) target = tabs[(i + 1) % tabs.length];
  else if (e.key === prev) target = tabs[(i - 1 + tabs.length) % tabs.length];
  else if (e.key === 'Home') target = tabs[0];
  else if (e.key === 'End') target = tabs.at(-1);
  else if (e.key === 'Delete') closeDockPanel(dock, t);
  else if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) openDockOptions(t.closest('.z-dock-area').querySelector('.z-dock-options'));
  else return false;
  e.preventDefault();
  e.stopPropagation();
  if (target) {
    selectDockTab(target, true);
    saveDock(dock);
  }
  return true;
}

/** Drags a divider with the pointer. */
function dragDockBar(e, bar) {
  const dock = bar.closest('.z-dock');
  const edge = bar.dataset.edge;
  bar.setPointerCapture(e.pointerId);
  bar.dataset.dragging = '';
  e.preventDefault();
  bar.focus();
  const onLeft = getComputedStyle(dock).direction === 'rtl' ? 'right' : 'left';
  const move = (ev) => {
    const box = dock.getBoundingClientRect();
    const percent =
      edge === onLeft ? ((ev.clientX - box.left) / box.width) * 100 : edge !== 'bottom' ? ((box.right - ev.clientX) / box.width) * 100 : ((box.bottom - ev.clientY) / box.height) * 100;
    setDockSize(dock, edge, percent);
  };
  const up = () => {
    delete bar.dataset.dragging;
    bar.removeEventListener('pointermove', move);
    bar.removeEventListener('pointerup', up);
    bar.removeEventListener('pointercancel', up);
    saveDock(dock);
  };
  bar.addEventListener('pointermove', move);
  bar.addEventListener('pointerup', up);
  bar.addEventListener('pointercancel', up);
}

/** Loads saved layouts: from the server when it keeps them, else from this browser. */
async function initDocks(root) {
  for (const dock of root.querySelectorAll('[data-zeyn-dock]')) {
    if (dock.zeynReady) continue;
    dock.zeynReady = true;
    refreshDock(dock);
    const r = await post('/_zeyn/dock', { ...dockAddress(dock), save: false });
    if (r.ok) {
      dock.dataset.store = 'server';
      applyDockLayout(dock, r.data?.layout);
    } else applyDockLayout(dock, load(dockStoreKey(dock), null));
  }
}

/** Fills the panels entry of a menu bar menu with the panels of the dock on its screen. */
function fillPanels(menu) {
  const slot = menu.querySelector(':scope > [data-zeyn-dock-panels]');
  if (!slot) return;
  menu.querySelectorAll(':scope > .z-dock-panels-item').forEach((li) => li.remove());
  const dock = (menu.closest('[data-zeyn-screen]') ?? document).querySelector('[data-zeyn-dock]');
  if (!dock) return;
  let after = slot;
  for (const tab of dockTabs(dock)) {
    const li = make('li', 'z-dock-panels-item', { role: 'none' });
    const b = make('button', 'z-menu-item', { type: 'button', role: 'menuitemcheckbox', tabindex: '-1', 'aria-checked': String(!tab.hidden) });
    b.append(make('span', 'z-menu-label', { text: dockTitle(tab) }));
    b.zeynDockTab = tab;
    li.append(b);
    after.after(li);
    after = li;
  }
}

/* Wiring ------------------------------------------------------------------------------------- */

document.addEventListener(
  'keydown',
  (e) => {
    if (e.defaultPrevented) return;
    const t = e.target instanceof Element ? e.target : null;
    if (!t) return;
    if (popKey(e, t)) return;
    if (t.closest('[data-zeyn-menubar]') && barKey(e, t)) return;
    if (t.matches('[data-zeyn-toolbox] .z-tool')) toolKey(e, t);
  },
  true,
);

/** Dock tabs skip closed panels, so they are handled before the generic tabs of components.js. */
window.addEventListener(
  'keydown',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    if (t && !e.defaultPrevented && t.matches('.z-dock-tab, .z-dock-bar') && dockKey(e, t)) e.stopPropagation();
  },
  true,
);

document.addEventListener('zeyn:menu-open', (e) => {
  if (e.detail instanceof Element) fillPanels(e.detail);
});

document.addEventListener('zeyn:menu-check', (e) => {
  const item = e.detail;
  const tab = item?.zeynDockTab;
  if (!tab?.isConnected) return;
  const dock = tab.closest('.z-dock');
  if (tab.hidden) showDockPanel(dock, tab, false);
  else closeDockPanel(dock, tab, false);
  item.setAttribute('aria-checked', String(!tab.hidden));
});

document.addEventListener('dragstart', (e) => {
  const tab = e.target instanceof Element ? e.target.closest('.z-dock-tab') : null;
  if (!tab || !e.dataTransfer) return;
  e.dataTransfer.setData('application/x-zeyn-dock', tab.id);
  e.dataTransfer.effectAllowed = 'move';
});

document.addEventListener('dragover', (e) => {
  const area = e.target instanceof Element ? e.target.closest('.z-dock-area') : null;
  if (!area || !e.dataTransfer?.types.includes('application/x-zeyn-dock')) return;
  e.preventDefault();
  e.dataTransfer.dropEffect = 'move';
  for (const a of area.parentElement.querySelectorAll(':scope > .z-dock-area')) a.classList.toggle('z-drop', a === area);
});

document.addEventListener('dragleave', (e) => {
  const area = e.target instanceof Element ? e.target.closest('.z-dock-area') : null;
  if (area && !(e.relatedTarget instanceof Element && area.contains(e.relatedTarget))) area.classList.remove('z-drop');
});

document.addEventListener('drop', (e) => {
  const area = e.target instanceof Element ? e.target.closest('.z-dock-area') : null;
  const tab = document.getElementById(e.dataTransfer?.getData('application/x-zeyn-dock') ?? '');
  document.querySelectorAll('.z-dock-area.z-drop').forEach((a) => a.classList.remove('z-drop'));
  if (!area || !tab?.matches('.z-dock-tab') || tab.closest('.z-dock') !== area.closest('.z-dock')) return;
  e.preventDefault();
  moveDockPanel(area.closest('.z-dock'), tab, area.dataset.area);
});

document.addEventListener('dragend', () => document.querySelectorAll('.z-dock-area.z-drop').forEach((a) => a.classList.remove('z-drop')));

document.addEventListener('zeyn:content', (e) => initDocks(e.detail instanceof Element ? e.detail : document));

new MutationObserver((records) => {
  for (const r of records) for (const n of r.addedNodes) if (n instanceof HTMLDialogElement) initDocks(n);
}).observe(document.body, { childList: true });

initDocks(document);

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
  if (!t || t.closest('[data-zeyn-palette-open], [data-zeyn-command-run]')) return;
  let el;
  if ((el = t.closest('.z-menubar-item'))) {
    if (el.getAttribute('aria-expanded') === 'true') closeBarMenus(el.closest('[data-zeyn-menubar]'));
    else openBarMenu(el, 'first');
    return;
  }
  if ((el = t.closest('[data-zeyn-toolbox] .z-tool'))) return pickTool(el);
  if ((el = t.closest('.z-pop-menu [role^="menuitem"]'))) return runPopItem(el);
  if ((el = t.closest('.z-dock-options'))) {
    if (popMenu?.opener === el) closePopMenu(true);
    else openDockOptions(el);
    return;
  }
  if ((el = t.closest('.z-dock-tab'))) {
    selectDockTab(el, true);
    return saveDock(el.closest('.z-dock'));
  }
});

document.addEventListener('pointerover', (e) => {
  const top = e.target instanceof Element ? e.target.closest('.z-menubar-item') : null;
  const bar = top?.closest('[data-zeyn-menubar]');
  if (bar && barOpen(bar) && top.getAttribute('aria-expanded') !== 'true') openBarMenu(top, 'none');
});

document.addEventListener(
  'pointerdown',
  (e) => {
    const t = e.target instanceof Element ? e.target : null;
    const inside = t?.closest('[data-zeyn-menubar]');
    for (const bar of document.querySelectorAll('[data-zeyn-menubar]')) if (bar !== inside && barOpen(bar)) closeBarMenus(bar);
    if (popMenu && !popMenu.menu.contains(t) && popMenu.opener !== t?.closest('.z-dock-options')) closePopMenu(false);
    const bar = t?.closest('.z-dock-bar');
    if (bar && e.button === 0) dragDockBar(e, bar);
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
