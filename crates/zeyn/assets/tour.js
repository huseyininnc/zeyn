/**
 * Guided tours (`zeyn::ui::Tour`). Loaded only by pages that hold `[data-zeyn-tour]`. It shows
 * the start button, which the server renders hidden, and walks a dialog over the visible
 * `.z-tourstop` elements of the page.
 * @module tour
 */

/** The most stops a tour visits (`MAX_TOUR_STOPS`). */
const MAX_STOPS = 30;

/** The running tour: its root, dialog, stops and position, or `null`. */
let run = null;

/** Shows the start buttons under `root`. */
function setup(root) {
  const found = root.matches?.('[data-zeyn-tour]') ? [root] : [...(root.querySelectorAll?.('[data-zeyn-tour]') ?? [])];
  for (const tour of found) {
    const start = tour.querySelector('.z-tour-start');
    if (start) start.hidden = false;
  }
}

/** Puts the dialog under the current stop, or above it when there is no room below. */
function place() {
  if (!run) return;
  const { dialog } = run;
  const at = run.stops[run.index].getBoundingClientRect();
  const width = dialog.offsetWidth;
  const height = dialog.offsetHeight;
  const gap = 12;
  const below = at.bottom + gap + height <= innerHeight;
  const above = at.top - gap - height >= 0;
  const y = below ? at.bottom + gap : above ? at.top - gap - height : innerHeight - height - gap;
  const rtl = getComputedStyle(dialog).direction === 'rtl';
  const start = rtl ? innerWidth - at.right : at.left;
  const x = Math.max(gap, Math.min(start, innerWidth - width - gap));
  dialog.style.setProperty('--z-tour-x', `${Math.round(x)}px`);
  dialog.style.setProperty('--z-tour-y', `${Math.round(Math.max(gap, y))}px`);
}

/** Shows stop `index`: its texts, the count and the mark on the page. */
function show(index) {
  const { dialog, stops } = run;
  stops[run.index]?.classList.remove('z-tour-on');
  run.index = index;
  const stop = stops[index];
  stop.classList.add('z-tour-on');
  const count = dialog.querySelector('.z-tour-count');
  count.textContent = (count.dataset.progress ?? '').replace('{n}', index + 1).replace('{total}', stops.length);
  dialog.querySelector('.z-tour-title').textContent = stop.dataset.tourTitle ?? '';
  dialog.querySelector('.z-tour-body').textContent = stop.dataset.tourBody ?? '';
  dialog.querySelector('.z-tour-back').hidden = index === 0;
  const next = dialog.querySelector('.z-tour-next');
  next.textContent = index === stops.length - 1 ? next.dataset.finish : next.dataset.next;
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  stop.scrollIntoView({ block: 'center', behavior: calm ? 'auto' : 'smooth' });
  place();
  next.focus();
}

/** Starts the tour of `tour` when the page has a visible stop. */
function open(tour) {
  const dialog = tour.querySelector('.z-tour-card');
  if (run && !run.dialog.isConnected) finish();
  const stops = [...(tour.closest('.z-screen') ?? document).querySelectorAll('.z-tourstop')].filter((s) => s.getClientRects().length > 0).slice(0, MAX_STOPS);
  if (!dialog || stops.length === 0 || run) return;
  run = { tour, dialog, stops, index: 0 };
  dialog.showModal();
  addEventListener('scroll', place, { passive: true, capture: true });
  addEventListener('resize', place);
  show(0);
}

/** Ends the tour and gives focus back to its start button. */
function finish() {
  if (!run) return;
  const { tour, dialog, stops } = run;
  stops[run.index]?.classList.remove('z-tour-on');
  removeEventListener('scroll', place, { capture: true });
  removeEventListener('resize', place);
  run = null;
  if (dialog.open) dialog.close();
  tour.querySelector('.z-tour-start')?.focus();
}

document.addEventListener('click', (e) => {
  const target = e.target instanceof Element ? e.target : null;
  const tour = target?.closest('[data-zeyn-tour]');
  if (!tour) return;
  if (target.closest('.z-tour-start')) open(tour);
  else if (!run) return;
  else if (target.closest('.z-tour-close')) finish();
  else if (target.closest('.z-tour-back')) show(Math.max(0, run.index - 1));
  else if (target.closest('.z-tour-next')) run.index === run.stops.length - 1 ? finish() : show(run.index + 1);
});

document.addEventListener('close', (e) => e.target === run?.dialog && finish(), true);
document.addEventListener('zeyn:content', (e) => setup(e.detail ?? document));
setup(document);
