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
 *
 * The rail button (`[data-zeyn-rail-toggle]`) folds the menu to an icon rail on wide screens and
 * back. In the rail a top level group opens as a flyout, one at a time; Escape, a click outside
 * and a navigation close it.
 * Counted items (`[data-zeyn-count]`, `Menu::count`) get their numbers from one
 * `GET /_zeyn/menu/counts` after the page shows, and again when one of their topics is named.
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

/** Topics named since the last count, so a burst of them counts once. */
let countTimer = 0;

/**
 * Fills the counted menu items (`[data-zeyn-count]`, see `Menu::count`) from one
 * `GET /_zeyn/menu/counts`: 0 hides the number, more than 99 shows "99+".
 */
async function loadCounts() {
  countTimer = 0;
  const spans = document.querySelectorAll('[data-zeyn-count]');
  if (!spans.length) return;
  const base = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
  let counts;
  try {
    const res = await fetch(`${base}/_zeyn/menu/counts`, { credentials: 'same-origin', cache: 'no-store' });
    if (!res.ok) return;
    counts = await res.json();
  } catch {
    return;
  }
  for (const span of spans) {
    const n = counts?.[span.dataset.zeynCount];
    const shown = typeof n === 'number' && n > 0;
    span.hidden = !shown;
    span.textContent = shown ? (n > 99 ? '99+' : String(n)) : '';
  }
}

document.addEventListener('zeyn:topics', (e) => {
  const named = e.detail?.topics ?? [];
  const wanted = [...document.querySelectorAll('[data-zeyn-count-topics]')].some((s) => s.dataset.zeynCountTopics.split(',').some((t) => named.includes(t)));
  if (wanted) countTimer ||= setTimeout(loadCounts, 300);
});

loadCounts();
