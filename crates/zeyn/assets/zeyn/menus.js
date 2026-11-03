/** Creates an element with a class and optional attributes. */
function make(tag, className, attrs = {}) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
  return node;
}

/** Places a popover under its button, inside the viewport, at the inline start edge. */
function place(panel, button) {
  const r = button.getBoundingClientRect();
  const rtl = getComputedStyle(button).direction === 'rtl';
  const width = panel.offsetWidth;
  const left = rtl ? r.right - width : r.left;
  panel.style.left = `${Math.max(4, Math.min(left, innerWidth - width - 4))}px`;
  const below = r.bottom + 2;
  panel.style.top = `${below + panel.offsetHeight > innerHeight ? Math.max(4, r.top - panel.offsetHeight - 2) : below}px`;
}

/** The menu that is open now, if any, with the element focus returns to. */
let openMenuState = null;

/**
 * Opens a menu (`role="menu"`) as an auto popover in the top layer, so no scroll container clips
 * it. Items: `{label, run, disabled, checked, radio, hint, items, separator}`; `items` makes a
 * submenu. Keyboard: arrows move (Right opens a submenu, Left closes it; mirrored in right to left
 * pages), Home and End jump, Enter and Space run, Escape closes, Tab closes. Focus returns to the
 * opener.
 * @param {object[]} items
 * @param {{anchor?: Element, x?: number, y?: number, label?: string, opener?: Element}} at
 */
function openMenu(items, at) {
  closeMenus();
  const opener = at.opener ?? at.anchor ?? document.activeElement;
  const host = dialogs.at(-1)?.dialog ?? document.body;
  const menu = buildMenu(items, at.label);
  host.append(menu);
  openMenuState = { menu, opener };
  at.anchor?.setAttribute('aria-expanded', 'true');
  menu.addEventListener('toggle', (e) => {
    if (e.newState !== 'closed') return;
    at.anchor?.setAttribute('aria-expanded', 'false');
    menu.remove();
    if (openMenuState?.menu === menu) openMenuState = null;
    if (!document.activeElement || document.activeElement === document.body || menu.contains(document.activeElement)) {
      if (opener?.isConnected) opener.focus();
    }
  });
  menu.showPopover();
  if (at.anchor) place(menu, at.anchor);
  else placeAt(menu, at.x ?? 0, at.y ?? 0);
  firstItem(menu)?.focus();
  return menu;
}

/** Closes the open menu, if any, without running anything. */
function closeMenus() {
  if (openMenuState?.menu.isConnected) openMenuState.menu.hidePopover();
  openMenuState = null;
}

/** Builds the menu element and its submenus. */
function buildMenu(items, label) {
  const menu = make('div', 'z-popover z-menu', { role: 'menu', popover: 'auto', tabindex: '-1' });
  if (label) menu.setAttribute('aria-label', label);
  for (const item of items) {
    if (item.separator) {
      menu.append(make('div', 'z-menu-sep', { role: 'separator' }));
      continue;
    }
    const role = item.checked == null ? 'menuitem' : item.radio ? 'menuitemradio' : 'menuitemcheckbox';
    const button = make('button', 'z-menu-item', { type: 'button', role, tabindex: '-1' });
    if (item.checked != null) button.setAttribute('aria-checked', String(!!item.checked));
    if (item.disabled) button.setAttribute('aria-disabled', 'true');
    if (item.icon) button.append(make('span', `z-icon z-icon-${item.icon}`, { 'aria-hidden': 'true' }));
    button.append(make('span', 'z-menu-label'));
    button.lastChild.textContent = item.label;
    if (item.hint) {
      const hint = make('span', 'z-menu-hint', { 'aria-hidden': 'true' });
      hint.textContent = item.hint;
      button.append(hint);
      button.setAttribute('aria-keyshortcuts', item.hint.replace('Ctrl', 'Control'));
    }
    if (item.items) {
      button.setAttribute('aria-haspopup', 'menu');
      button.setAttribute('aria-expanded', 'false');
      const sub = buildMenu(item.items, item.label);
      sub.popover = 'auto';
      button._submenu = sub;
      sub.addEventListener('toggle', (e) => button.setAttribute('aria-expanded', String(e.newState === 'open')));
    }
    button._item = item;
    menu.append(button);
    if (button._submenu) menu.append(button._submenu);
  }
  return menu;
}

/** Enabled items of one menu level. */
function menuItems(menu) {
  return [...menu.children].filter((c) => c.matches('.z-menu-item'));
}

function firstItem(menu) {
  return menuItems(menu).find((b) => b.getAttribute('aria-disabled') !== 'true') ?? menuItems(menu)[0];
}

/** Places a popover at a point (a right click), kept inside the viewport. */
function placeAt(panel, x, y) {
  const w = panel.offsetWidth;
  const h = panel.offsetHeight;
  panel.style.left = `${Math.max(4, Math.min(x, innerWidth - w - 4))}px`;
  panel.style.top = `${Math.max(4, Math.min(y, innerHeight - h - 4))}px`;
}

/** Opens the submenu of an item beside it and focuses its first item. */
function openSubmenu(button) {
  const sub = button._submenu;
  if (!sub) return;
  sub.showPopover();
  const r = button.getBoundingClientRect();
  const rtl = getComputedStyle(button).direction === 'rtl';
  const x = rtl ? r.left - sub.offsetWidth : r.right;
  placeAt(sub, x, r.top - 4);
  firstItem(sub)?.focus();
}

/** Runs a menu item: opens its submenu or runs it and closes every menu. */
function runMenuItem(button) {
  if (button.getAttribute('aria-disabled') === 'true') return;
  if (button._submenu) return openSubmenu(button);
  const item = button._item;
  const opener = openMenuState?.opener;
  closeMenus();
  if (opener?.isConnected) opener.focus();
  item?.run?.();
}

/** Keyboard inside a menu. Returns true when handled. */
function menuKey(e, button) {
  const menu = button.parentElement;
  const list = menuItems(menu);
  const i = list.indexOf(button);
  const rtl = getComputedStyle(menu).direction === 'rtl';
  const into = rtl ? 'ArrowLeft' : 'ArrowRight';
  const out = rtl ? 'ArrowRight' : 'ArrowLeft';
  let target = null;
  if (e.key === 'ArrowDown') target = list[(i + 1) % list.length];
  else if (e.key === 'ArrowUp') target = list[(i - 1 + list.length) % list.length];
  else if (e.key === 'Home') target = list[0];
  else if (e.key === 'End') target = list.at(-1);
  else if (e.key === into && button._submenu) {
    openSubmenu(button);
  } else if (e.key === out && menu.parentElement?.closest('.z-menu')) {
    const parent = menu.previousElementSibling;
    menu.hidePopover();
    parent?.focus();
  } else if (e.key === 'Enter' || e.key === ' ') {
    runMenuItem(button);
  } else if (e.key === 'Escape') {
    if (menu.parentElement?.closest('.z-menu')) {
      menu.hidePopover();
      menu.previousElementSibling?.focus();
    } else closeMenus();
  } else if (e.key === 'Tab') {
    closeMenus();
    return false;
  } else return false;
  e.preventDefault();
  e.stopPropagation();
  target?.focus();
  return true;
}

/** Relative time formatter for the page language, made once. */
let relativeFormat = null;

/**
 * Shows `time[data-zeyn-relative]` as relative time ("3 minutes ago") in the page language. The
 * server rendered exact time stays in the `title`. Called when rows arrive and once a minute.
 */
function relativeTimes(root) {
  const list = root.querySelectorAll('time[data-zeyn-relative]');
  if (!list.length || typeof Intl.RelativeTimeFormat !== 'function') return;
  relativeFormat ??= new Intl.RelativeTimeFormat(document.documentElement.lang || undefined, { numeric: 'auto' });
  const now = Date.now();
  const units = [
    ['year', 31536000],
    ['month', 2592000],
    ['week', 604800],
    ['day', 86400],
    ['hour', 3600],
    ['minute', 60],
  ];
  for (const el of list) {
    const at = Date.parse(el.getAttribute('datetime'));
    if (Number.isNaN(at)) continue;
    const secs = (at - now) / 1000;
    const [unit, size] = units.find(([, n]) => Math.abs(secs) >= n) ?? ['second', 1];
    el.textContent = relativeFormat.format(Math.round(secs / size), unit);
  }
}

