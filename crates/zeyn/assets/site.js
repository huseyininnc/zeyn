/**
 * Site shell runtime.
 *
 * The top menu follows the W3C APG disclosure navigation pattern: each group is a button with
 * `aria-expanded` that shows or hides its list of links. Enter, Space or a click toggle a group,
 * Down on a group button opens it and moves to its first link, Up and Down move between the links
 * of an open group, Home and End jump to the first and last, Escape closes the group and returns
 * to its button, and leaving the group with Tab or a click outside closes it. Left and Right move
 * between the top level items. Below 800px the menu folds behind the menu button, which this
 * script shows; without the script the menu stays open, so nothing becomes unreachable.
 * @module site
 */

/** Groups of the top menu. */
function groups() {
  return [...document.querySelectorAll('.z-site-group-button')];
}

/** Closes every open group except `keep`. */
function closeGroups(keep) {
  for (const b of groups()) {
    if (b === keep || b.getAttribute('aria-expanded') !== 'true') continue;
    b.setAttribute('aria-expanded', 'false');
    document.getElementById(b.getAttribute('aria-controls'))?.setAttribute('hidden', '');
  }
}

/** Opens or closes one group. */
function setGroup(button, open) {
  if (open) closeGroups(button);
  button.setAttribute('aria-expanded', String(open));
  const list = document.getElementById(button.getAttribute('aria-controls'));
  if (open) list?.removeAttribute('hidden');
  else list?.setAttribute('hidden', '');
  return list;
}

/** Opens or closes the folded menu on narrow screens. */
function setMenu(open) {
  const toggle = document.querySelector('[data-zeyn-site-toggle]');
  const nav = toggle && document.getElementById(toggle.getAttribute('aria-controls'));
  if (!toggle || !nav) return;
  toggle.setAttribute('aria-expanded', String(open));
  if (open) nav.setAttribute('data-open', '');
  else nav.removeAttribute('data-open');
}

/** Shows the menu button, so the menu can fold on narrow screens, and marks the group that
 * holds the current page. */
function init() {
  for (const b of groups()) {
    const list = document.getElementById(b.getAttribute('aria-controls'));
    b.toggleAttribute('data-current', Boolean(list?.querySelector('a[aria-current="page"]')));
  }
  const toggle = document.querySelector('[data-zeyn-site-toggle]');
  if (!toggle || !toggle.hidden) return;
  toggle.hidden = false;
  toggle.closest('[data-zeyn-site-header]')?.setAttribute('data-zeyn-collapsible', '');
}

/** Top level items of the menu: links and group buttons. */
function topItems() {
  return [...document.querySelectorAll('.z-site-menu > li > a, .z-site-menu > li > .z-site-group-button')];
}

document.addEventListener('click', (e) => {
  const toggle = e.target.closest?.('[data-zeyn-site-toggle]');
  if (toggle) {
    setMenu(toggle.getAttribute('aria-expanded') !== 'true');
    return;
  }
  const button = e.target.closest?.('.z-site-group-button');
  if (button) {
    setGroup(button, button.getAttribute('aria-expanded') !== 'true');
    return;
  }
  if (e.target.closest?.('.z-site-nav a')) {
    closeGroups();
    setMenu(false);
    return;
  }
  if (!e.target.closest?.('.z-site-group')) closeGroups();
});

document.addEventListener('keydown', (e) => {
  const nav = e.target.closest?.('.z-site-nav');
  if (!nav) return;
  const group = e.target.closest('.z-site-group');
  const button = group?.querySelector('.z-site-group-button');
  const links = group ? [...group.querySelectorAll('.z-site-sub a')] : [];
  const inList = e.target.closest('.z-site-sub') !== null;
  if (e.key === 'Escape') {
    if (button && button.getAttribute('aria-expanded') === 'true') {
      setGroup(button, false);
      button.focus();
      e.preventDefault();
    } else if (document.querySelector('[data-zeyn-site-toggle][aria-expanded="true"]')) {
      setMenu(false);
      document.querySelector('[data-zeyn-site-toggle]')?.focus();
      e.preventDefault();
    }
    return;
  }
  if (inList && links.length) {
    const at = links.indexOf(e.target);
    const next = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: links.length - 1 }[e.key];
    if (next === undefined) return;
    e.preventDefault();
    links[Math.max(0, Math.min(links.length - 1, next))].focus();
    return;
  }
  if (e.target === button && e.key === 'ArrowDown') {
    e.preventDefault();
    setGroup(button, true);
    links[0]?.focus();
    return;
  }
  const top = topItems();
  const at = top.indexOf(e.target);
  if (at < 0) return;
  const rtl = e.target.closest('[dir="rtl"]') !== null;
  const step = { ArrowRight: rtl ? -1 : 1, ArrowLeft: rtl ? 1 : -1 }[e.key];
  const jump = { Home: 0, End: top.length - 1 }[e.key];
  if (step === undefined && jump === undefined) return;
  e.preventDefault();
  top[jump ?? (at + step + top.length) % top.length].focus();
});

document.addEventListener('focusout', (e) => {
  const group = e.target.closest?.('.z-site-group');
  if (group && !group.contains(e.relatedTarget)) {
    const button = group.querySelector('.z-site-group-button');
    if (button?.getAttribute('aria-expanded') === 'true' && e.relatedTarget) setGroup(button, false);
  }
});

document.addEventListener('zeyn:leave', () => {
  closeGroups();
  setMenu(false);
});
document.addEventListener('zeyn:content', init);
init();
