/**
 * Heroes with slides (`zeyn::ui::Hero`), after the W3C APG carousel pattern. components.js loads
 * this module the first time a page holds one. The server renders the first slide and hides the
 * others and the controls, so without script the first slide shows. This module shows the slide
 * picker, and for heroes the app asked to rotate (`data-rotate`, seconds) it moves to the next
 * slide on a timer with a pause and play button first (WCAG 2.2.2). Rotation stops while the
 * pointer is over the hero or focus is inside it, while the tab is hidden, and never starts
 * when the user prefers reduced motion. While it rotates, the slides are not announced
 * (`aria-live="off"`); when it stops, a picked slide is announced politely.
 * @module hero
 */

const reduced = matchMedia('(prefers-reduced-motion: reduce)');

/** @type {WeakMap<Element, {index: number, playing: boolean, held: boolean, timer: number}>} */
const heroes = new WeakMap();

/** Shows slide `index` of a hero and marks its picker button. */
function show(hero, index) {
  const s = heroes.get(hero);
  const slides = hero.querySelectorAll(':scope > .z-hero-track > .z-hero-slide');
  if (!s || !slides.length) return;
  s.index = (index + slides.length) % slides.length;
  slides.forEach((slide, i) => (slide.hidden = i !== s.index));
  hero.querySelectorAll('[data-zeyn-hero-show]').forEach((b, i) => {
    if (i === s.index) b.setAttribute('aria-current', 'true');
    else b.removeAttribute('aria-current');
  });
}

/** Writes the playing state into the pause button and the live region. */
function sync(hero) {
  const s = heroes.get(hero);
  const button = hero.querySelector('[data-zeyn-hero-pause]');
  const moving = s.playing && !s.held;
  hero.querySelector(':scope > .z-hero-track')?.setAttribute('aria-live', moving ? 'off' : 'polite');
  if (!button) return;
  button.setAttribute('aria-label', s.playing ? button.dataset.textPause : button.dataset.textPlay);
  button.toggleAttribute('data-playing', s.playing);
}

/** One step of the timer of a hero: the next slide, unless it is held, hidden or gone. */
function tick(hero) {
  const s = heroes.get(hero);
  if (!hero.isConnected) {
    clearInterval(s.timer);
    return;
  }
  if (s.playing && !s.held && document.visibilityState === 'visible') show(hero, s.index + 1);
}

/** Sets up the heroes with slides inside `root`. */
function init(root) {
  for (const hero of root.querySelectorAll('[data-zeyn-hero]')) {
    if (heroes.has(hero)) continue;
    const seconds = Number(hero.dataset.rotate) || 0;
    const s = { index: 0, playing: seconds > 0 && !reduced.matches, held: false, timer: 0 };
    heroes.set(hero, s);
    if (seconds > 0) s.timer = setInterval(() => tick(hero), seconds * 1000);
    const controls = hero.querySelector(':scope > .z-hero-controls');
    if (controls) controls.hidden = false;
    hero.addEventListener('pointerenter', () => hold(hero, true));
    hero.addEventListener('pointerleave', () => hold(hero, hero.contains(document.activeElement)));
    hero.addEventListener('focusin', () => hold(hero, true));
    hero.addEventListener('focusout', (e) => {
      if (!hero.contains(e.relatedTarget)) hold(hero, hero.matches(':hover'));
    });
    sync(hero);
  }
}

/** Holds or releases the rotation of a hero for hover and focus. */
function hold(hero, held) {
  const s = heroes.get(hero);
  if (!s) return;
  s.held = held;
  sync(hero);
}

document.addEventListener('click', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  const hero = t?.closest('[data-zeyn-hero]');
  const s = hero && heroes.get(hero);
  if (!s) return;
  const pick = t.closest('[data-zeyn-hero-show]');
  if (pick) show(hero, Number(pick.dataset.zeynHeroShow));
  else if (t.closest('[data-zeyn-hero-pause]')) {
    s.playing = !s.playing;
    sync(hero);
  }
});

reduced.addEventListener('change', () => {
  if (!reduced.matches) return;
  for (const hero of document.querySelectorAll('[data-zeyn-hero]')) {
    const s = heroes.get(hero);
    if (!s) continue;
    s.playing = false;
    sync(hero);
  }
});

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
