/**
 * Menu buttons and split buttons (W3C APG menu button). Loaded only by pages that hold one
 * (`.z-menu-trigger`, `.z-split-button`).
 * @module menubutton
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { clickHandlers, keyHandlers, typeAhead } = K;

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
