/**
 * Calendars (`zeyn::ui::Calendar`). Loaded only by pages that hold `[data-zeyn-calendar]`.
 *
 * The server renders every view; the toolbar links change the screen params through
 * `topics.js`. This module adds the keyboard of the APG grid pattern and choosing slots. The
 * grid is one tab stop: arrow keys move between cells (Left and Right swap under `dir="rtl"`),
 * Home and End go to the first and last cell of the row, Page Up and Page Down follow the
 * previous and next links, Enter moves into the events of the cell (Up and Down between them,
 * Escape back to the cell). With a create action (`data-create`), N or Enter on an empty cell
 * creates in the chosen slot, Shift with Up or Down (hours) or Left and Right (days) widens it,
 * and a pointer press and drag over cells chooses them. The slot goes to `POST /_zeyn/data`
 * (key `create`) as local wall times; the server reads them in the user's zone, runs the action
 * and answers with an effect, which runs, and the calendar renders again through its topic.
 * @module calendar
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/** Parses JSON, or returns `null`. */
function parse(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** The cells of a calendar grid by row, header cells left out. */
function rows(cal) {
  return [...cal.querySelectorAll('.z-cal-grid tbody tr')].map((tr) => [...tr.querySelectorAll('[role="gridcell"]')]);
}

/** Makes `cell` the one tab stop of the grid and focuses it. */
function focusCell(cell) {
  const cal = cell.closest('[data-zeyn-calendar]');
  for (const c of cal.querySelectorAll('[role="gridcell"][tabindex="0"]')) c.tabIndex = -1;
  cell.tabIndex = 0;
  cell.focus();
}

/** Left and Right as they read: swapped under `dir="rtl"`. */
function reading(key, el) {
  if (getComputedStyle(el).direction !== 'rtl') return key;
  return key === 'ArrowLeft' ? 'ArrowRight' : key === 'ArrowRight' ? 'ArrowLeft' : key;
}

/** The chosen cells of a calendar: an anchor and the current cell, same column for hours. */
const chosen = new WeakMap();

/** Marks the cells between the anchor and `to`. */
function choose(cal, anchor, to) {
  const grid = rows(cal);
  const at = (c) => {
    for (let r = 0; r < grid.length; r++) {
      const i = grid[r].indexOf(c);
      if (i >= 0) return [r, i];
    }
    return [-1, -1];
  };
  const [ar, ac] = at(anchor);
  const [tr, tc] = at(to);
  const month = cal.dataset.view === 'month';
  const allDay = !month && (anchor.dataset.day || to.dataset.day);
  const cells = [];
  if (month) {
    const flat = grid.flat();
    const a = flat.indexOf(anchor);
    const b = flat.indexOf(to);
    cells.push(...flat.slice(Math.min(a, b), Math.max(a, b) + 1));
  } else if (allDay) {
    const row = grid[0];
    cells.push(...row.slice(Math.min(ac, tc), Math.max(ac, tc) + 1));
  } else {
    for (let r = Math.min(ar, tr); r <= Math.max(ar, tr); r++) if (grid[r][ac]?.dataset.slot) cells.push(grid[r][ac]);
  }
  for (const c of cal.querySelectorAll('.z-cal-chosen')) c.classList.remove('z-cal-chosen');
  for (const c of cells) c.classList.add('z-cal-chosen');
  chosen.set(cal, { anchor, to, cells });
}

/** The slot of chosen cells as local wall times. */
function slotOf(cells) {
  const days = cells.map((c) => c.dataset.day).filter(Boolean).sort();
  if (days.length) return { start: days[0], end: days.at(-1), all_day: true };
  const slots = cells.map((c) => c.dataset.slot).sort();
  if (!slots.length) return null;
  const [d, t] = slots.at(-1).split('T');
  const hour = Number(t.slice(0, 2)) + 1;
  const end = hour === 24 ? `${nextDay(d)}T00:00` : `${d}T${String(hour).padStart(2, '0')}:00`;
  return { start: slots[0], end, all_day: false };
}

/** The day after a `YYYY-MM-DD` date. */
function nextDay(d) {
  const t = new Date(`${d}T00:00:00Z`);
  t.setUTCDate(t.getUTCDate() + 1);
  return t.toISOString().slice(0, 10);
}

/** Runs the create action for the chosen cells. */
async function create(cal) {
  const sel = chosen.get(cal);
  const cell = document.activeElement?.closest?.('[role="gridcell"]');
  const cells = sel?.cells?.length ? sel.cells : cell ? [cell] : [];
  const slot = slotOf(cells);
  if (!slot || !cal.hasAttribute('data-create')) return;
  const screen = cal.closest('[data-zeyn-screen]');
  let res = null;
  try {
    const r = await fetch(BASE + '/_zeyn/data', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify({
        screen: screen?.dataset.zeynScreen ?? null,
        params: parse(screen?.dataset.zeynParams ?? 'null'),
        node: cal.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
        key: 'create',
        input: slot,
      }),
    });
    res = { ok: r.ok, body: parse(await r.text()) };
  } catch {
    res = { ok: false, body: null };
  }
  for (const c of cal.querySelectorAll('.z-cal-chosen')) c.classList.remove('z-cal-chosen');
  chosen.delete(cal);
  if (!res.ok) {
    const message = res.body?.errors?.[0]?.message || res.body?.message || parse(cal.dataset.texts ?? '{}')?.failed;
    document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects: [{ effect: 'toast', message, tone: 'danger' }] } }));
    return;
  }
  const effects = res.body?.effect ? [res.body.effect] : [];
  document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects } }));
  document.dispatchEvent(new CustomEvent('zeyn:topics', { detail: { topics: (cal.dataset.zeynListens ?? '').split(',').filter(Boolean) } }));
}

/** Follows the previous or next link of the toolbar. */
function page(cal, forward) {
  const links = cal.querySelectorAll('.z-cal-nav a');
  links[forward ? 2 : 0]?.click();
}

document.addEventListener('keydown', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  const cal = t?.closest('[data-zeyn-calendar]');
  if (!cal) return;
  const event = t.closest('.z-cal-event');
  if (event) {
    const cell = event.closest('[role="gridcell"]');
    const list = [...cell.querySelectorAll('.z-cal-event')];
    const i = list.indexOf(event);
    if (e.key === 'Escape') focusCell(cell);
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') list[i + (e.key === 'ArrowDown' ? 1 : -1)]?.focus();
    else if (e.key === 'Enter' && event.tagName !== 'BUTTON') return;
    else return;
    e.preventDefault();
    return;
  }
  const cell = t.closest('[role="gridcell"]');
  if (!cell || t !== cell) return;
  const grid = rows(cal);
  let r = grid.findIndex((row) => row.includes(cell));
  let c = grid[r].indexOf(cell);
  const key = reading(e.key, cell);
  const month = cal.dataset.view === 'month';
  switch (key) {
    case 'ArrowRight':
      c++;
      break;
    case 'ArrowLeft':
      c--;
      break;
    case 'ArrowDown':
      r++;
      break;
    case 'ArrowUp':
      r--;
      break;
    case 'Home':
      c = 0;
      break;
    case 'End':
      c = grid[r].length - 1;
      break;
    case 'PageUp':
    case 'PageDown':
      e.preventDefault();
      page(cal, key === 'PageDown');
      return;
    case 'Enter':
    case ' ': {
      e.preventDefault();
      const first = cell.querySelector('.z-cal-event');
      if (first && !chosen.get(cal)) first.focus();
      else create(cal);
      return;
    }
    case 'n':
    case 'N':
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      e.preventDefault();
      create(cal);
      return;
    case 'Escape':
      for (const x of cal.querySelectorAll('.z-cal-chosen')) x.classList.remove('z-cal-chosen');
      chosen.delete(cal);
      return;
    default:
      return;
  }
  e.preventDefault();
  if (month && (c < 0 || c >= 7)) {
    r += c < 0 ? -1 : 1;
    c = c < 0 ? 6 : 0;
  }
  const next = grid[r]?.[Math.max(0, Math.min(c, (grid[r]?.length ?? 1) - 1))];
  if (!next) return;
  if (e.shiftKey && cal.hasAttribute('data-create')) {
    const sel = chosen.get(cal);
    choose(cal, sel?.anchor ?? cell, next);
  } else if (!e.shiftKey) {
    for (const x of cal.querySelectorAll('.z-cal-chosen')) x.classList.remove('z-cal-chosen');
    chosen.delete(cal);
  }
  focusCell(next);
});

let press = null;

document.addEventListener('pointerdown', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  const cell = t?.closest('[data-zeyn-calendar][data-create] [role="gridcell"]');
  if (!cell || e.button !== 0 || t.closest('.z-cal-event, a, button')) return;
  press = { cal: cell.closest('[data-zeyn-calendar]'), anchor: cell, moved: false };
  choose(press.cal, cell, cell);
});

document.addEventListener('pointermove', (e) => {
  if (!press) return;
  const cell = document.elementFromPoint(e.clientX, e.clientY)?.closest('[role="gridcell"]');
  if (cell && press.cal.contains(cell)) {
    press.moved = true;
    choose(press.cal, press.anchor, cell);
  }
});

document.addEventListener('pointerup', () => {
  if (!press) return;
  const { cal } = press;
  press = null;
  focusCell(chosen.get(cal)?.to ?? cal.querySelector('[role="gridcell"]'));
  create(cal);
});

document.addEventListener('pointercancel', () => {
  if (!press) return;
  for (const x of press.cal.querySelectorAll('.z-cal-chosen')) x.classList.remove('z-cal-chosen');
  press = null;
});
