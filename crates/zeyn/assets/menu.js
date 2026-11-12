/**
 * Collapsible groups of the app shell's side menu (`[data-zeyn-menu]`), the W3C APG disclosure
 * navigation pattern. A group heading is a button with `aria-expanded` that shows or hides the
 * group's list; the CSS hides closed lists only while scripting is on, so without this script
 * every group stays open. In an accordion menu (`[data-zeyn-accordion]`) opening a group closes
 * its open siblings. `nav.js` opens the branch of the current link after each navigation.
 *
 * Keys, with focus on a heading button or a link of the menu: Up and Down move between the
 * visible buttons and links, Home and End go to the first and the last; Right opens a closed
 * group or moves into an open one; Left closes an open group or moves to the heading of the
 * group around it. Enter and Space toggle a group (native button). Left and Right swap in right
 * to left languages.
 * A long menu (`[data-zeyn-menu-filter]`) gets a filter box: typing hides the links whose text
 * does not hold what was typed (case and accents ignored), opens the groups around the ones that
 * do, and says how many are left. Escape clears it. Each link gets a pin button; pinned links show
 * in a Favorites group at the top, kept in `localStorage` per user (`meta[name=zeyn-storage]`).
 *
 * The rail button (`[data-zeyn-rail-toggle]`) folds the menu to an icon rail on wide screens and
 * back. In the rail a top level group opens as a flyout, one at a time; Escape, a click outside
 * and a navigation close it.
 * @module menu
 */

/** The heading button of a `.z-nav-group`, or `null` when the group stays open. */
function heading(group) {
  return group?.querySelector(':scope > :is(h2, h3, h4, h5) > .z-nav-heading') ?? null;
}

/** The top level group headings of the icon rail, whose groups open as flyouts. */
function flyouts() {
  return document.querySelectorAll('[data-zeyn-rail] .z-nav-group > h2 > .z-nav-heading');
}

/** Closes every open flyout of the icon rail; `true` when one was open. */
function closeFlyouts() {
  let closed = false;
  for (const b of flyouts()) {
    if (b.getAttribute('aria-expanded') === 'true') closed = true;
    b.setAttribute('aria-expanded', 'false');
  }
  return closed;
}

/**
 * Folds the side menu to the icon rail or unfolds it, and remembers the choice in the `zeyn_rail`
 * cookie through `POST /_zeyn/rail`, so the next page starts the same way.
 */
async function setRail(button, on) {
  const app = button.closest('.z-app');
  app?.toggleAttribute('data-zeyn-rail', on);
  button.setAttribute('aria-pressed', String(on));
  button.setAttribute('aria-label', on ? button.dataset.textExpand : button.dataset.textCollapse);
  if (on) closeFlyouts();
  else {
    const current = app?.querySelector('.z-nav a[aria-current="page"]');
    for (let g = current?.closest('.z-nav-group'); g; g = g.parentElement.closest('.z-nav-group')) heading(g)?.setAttribute('aria-expanded', 'true');
  }
  const base = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
  const csrf = document.querySelector('meta[name="zeyn-csrf"]')?.content || (await window.zeynCsrf?.()) || '';
  fetch(`${base}/_zeyn/rail`, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'content-type': 'application/json', 'x-zeyn-csrf': csrf },
    body: JSON.stringify({ rail: on }),
  }).catch(() => {});
}

/**
 * Opens or closes the group of `button`; in an accordion menu, and among the flyouts of the icon
 * rail, opening closes its siblings.
 */
function setOpen(button, open) {
  button.setAttribute('aria-expanded', String(open));
  const group = button.closest('.z-nav-group');
  const flyout = !!group?.closest('[data-zeyn-rail]') && group.parentElement?.classList.contains('z-nav-body');
  if (!open || (!flyout && !group?.closest('[data-zeyn-accordion]'))) return;
  for (const sibling of group.parentElement.children) {
    if (sibling !== group) heading(sibling)?.setAttribute('aria-expanded', 'false');
  }
}

document.addEventListener('click', (e) => {
  const target = e.target instanceof Element ? e.target : null;
  const rail = target?.closest('[data-zeyn-rail-toggle]');
  if (rail) {
    setRail(rail, rail.getAttribute('aria-pressed') !== 'true');
    return;
  }
  const button = target?.closest('[data-zeyn-menu] .z-nav-heading');
  if (button) setOpen(button, button.getAttribute('aria-expanded') !== 'true');
  else if (!target?.closest('[data-zeyn-rail] .z-nav-group > ul')) closeFlyouts();
});

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && e.target instanceof Element && e.target.closest('[data-zeyn-rail] .z-nav-group')) {
    const group = e.target.closest('[data-zeyn-rail] .z-nav-body > .z-nav-group');
    const button = group && heading(group);
    if (button?.getAttribute('aria-expanded') === 'true') {
      setOpen(button, false);
      button.focus();
      e.preventDefault();
      return;
    }
  }
  if (e.altKey || e.ctrlKey || e.metaKey || !(e.target instanceof Element)) return;
  const nav = e.target.closest('[data-zeyn-menu]');
  const here = nav && e.target.closest('.z-nav-group') ? e.target.closest('.z-nav-heading, a[href]') : null;
  if (!here) return;
  const rtl = getComputedStyle(nav).direction === 'rtl';
  const key = rtl ? { ArrowLeft: 'ArrowRight', ArrowRight: 'ArrowLeft' }[e.key] ?? e.key : e.key;
  const all = [...nav.querySelectorAll('.z-nav-group .z-nav-heading, .z-nav-group a[href]')].filter((el) => el.checkVisibility());
  const at = all.indexOf(here);
  const isButton = here.classList.contains('z-nav-heading');
  const expanded = here.getAttribute('aria-expanded') === 'true';
  let next = null;
  if (key === 'ArrowDown') next = all[at + 1];
  else if (key === 'ArrowUp') next = all[at - 1];
  else if (key === 'Home') next = all[0];
  else if (key === 'End') next = all[all.length - 1];
  else if (key === 'ArrowRight' && isButton) {
    if (expanded) next = all[at + 1];
    else setOpen(here, true);
  } else if (key === 'ArrowLeft') {
    if (isButton && expanded) setOpen(here, false);
    else {
      const group = here.closest('.z-nav-group');
      next = heading(isButton ? group.parentElement.closest('.z-nav-group') : group);
    }
  } else if (key !== 'ArrowRight') return;
  e.preventDefault();
  next?.focus();
});

/** Text for comparing: lower case, without accents. */
function fold(text) {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase();
}

/** `localStorage` key of the user's pinned menu links. */
function favoritesKey() {
  return `zeyn.menu.favorites:${document.querySelector('meta[name="zeyn-storage"]')?.content ?? 'anon'}`;
}

/** The hrefs the user pinned, at most 20. */
function favorites() {
  try {
    const list = JSON.parse(localStorage.getItem(favoritesKey()) ?? '[]');
    return Array.isArray(list) ? list.filter((h) => typeof h === 'string').slice(0, 20) : [];
  } catch {
    return [];
  }
}

function saveFavorites(list) {
  try {
    localStorage.setItem(favoritesKey(), JSON.stringify(list.slice(0, 20)));
  } catch {
    return;
  }
}

/** The links of the menu's own groups, without the Favorites group. */
function menuLinks(nav) {
  return [...nav.querySelectorAll('.z-nav-group:not(.z-nav-favorites) a[href]')];
}

/** Draws the Favorites group and the state of every pin button. */
function drawFavorites(nav, box) {
  const pinned = favorites();
  nav.querySelector('.z-nav-favorites')?.remove();
  const links = menuLinks(nav);
  for (const a of links) {
    const button = a.parentElement.querySelector(':scope > .z-nav-pin');
    if (!button) continue;
    const on = pinned.includes(a.getAttribute('href'));
    const label = a.textContent.trim();
    button.setAttribute('aria-pressed', String(on));
    button.setAttribute('aria-label', (on ? box.dataset.textUnpin : box.dataset.textPin).replace('{label}', label));
  }
  const shown = pinned.map((h) => links.find((a) => a.getAttribute('href') === h)).filter(Boolean);
  if (!shown.length) return;
  const group = document.createElement('div');
  group.className = 'z-nav-group z-nav-favorites';
  const heading = document.createElement('h2');
  heading.textContent = box.dataset.textFavorites;
  const list = document.createElement('ul');
  for (const a of shown) {
    const li = document.createElement('li');
    const copy = a.cloneNode(true);
    copy.removeAttribute('id');
    copy.querySelector('.z-nav-count')?.remove();
    li.append(copy);
    list.append(li);
  }
  group.append(heading, list);
  box.after(group);
}

/** Adds the pin buttons and wires the filter box of a long menu. */
function initFilter(box) {
  if (box.dataset.zeynReady != null) return;
  box.dataset.zeynReady = '';
  const nav = box.closest('.z-nav');
  const input = box.querySelector('input');
  const status = box.querySelector('[role="status"]');
  for (const a of menuLinks(nav)) {
    const pin = document.createElement('button');
    pin.type = 'button';
    pin.className = 'z-nav-pin';
    a.after(pin);
  }
  drawFavorites(nav, box);
  const apply = () => {
    const q = fold(input.value.trim());
    let shown = 0;
    for (const li of nav.querySelectorAll('.z-nav-group:not(.z-nav-favorites) li:has(> a[href])')) {
      const hit = !q || fold(li.querySelector(':scope > a').textContent).includes(q);
      li.hidden = !hit;
      if (hit) shown++;
    }
    for (const g of nav.querySelectorAll('.z-nav-group:not(.z-nav-favorites)')) {
      g.hidden = !!q && !g.querySelector('li:has(> a[href]):not([hidden])');
      if (q && !g.hidden) heading(g)?.setAttribute('aria-expanded', 'true');
    }
    status.textContent = q ? (shown ? box.dataset.textResults.replace('{n}', String(shown)) : box.dataset.textNone) : '';
  };
  input.addEventListener('input', apply);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && input.value) {
      input.value = '';
      apply();
      e.preventDefault();
      e.stopPropagation();
    }
  });
  nav.addEventListener('click', (e) => {
    const pin = e.target instanceof Element ? e.target.closest('.z-nav-pin') : null;
    if (!pin) return;
    const href = pin.previousElementSibling?.getAttribute('href');
    if (!href) return;
    const list = favorites();
    const at = list.indexOf(href);
    if (at >= 0) list.splice(at, 1);
    else list.unshift(href);
    saveFavorites(list);
    drawFavorites(nav, box);
    nav.querySelector(`.z-nav-group:not(.z-nav-favorites) a[href="${CSS.escape(href)}"] + .z-nav-pin`)?.focus();
  });
}

for (const box of document.querySelectorAll('[data-zeyn-menu-filter]')) initFilter(box);
