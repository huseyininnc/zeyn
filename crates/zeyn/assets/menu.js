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
 * @module menu
 */

/** The heading button of a `.z-nav-group`, or `null` when the group stays open. */
function heading(group) {
  return group?.querySelector(':scope > :is(h2, h3, h4, h5) > .z-nav-heading') ?? null;
}

/** Opens or closes the group of `button`; in an accordion menu opening closes its siblings. */
function setOpen(button, open) {
  button.setAttribute('aria-expanded', String(open));
  const group = button.closest('.z-nav-group');
  if (!open || !group?.closest('[data-zeyn-accordion]')) return;
  for (const sibling of group.parentElement.children) {
    if (sibling !== group) heading(sibling)?.setAttribute('aria-expanded', 'false');
  }
}

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('[data-zeyn-menu] .z-nav-heading') : null;
  if (button) setOpen(button, button.getAttribute('aria-expanded') !== 'true');
});

document.addEventListener('keydown', (e) => {
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
