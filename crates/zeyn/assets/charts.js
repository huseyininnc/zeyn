/**
 * Zeyn charts.
 *
 * Keyboard and tooltips for `zeyn::ui::Chart`. The chart is one tab stop: the first data mark
 * has `tabindex="0"`, the others `-1`. Left and Right move along a series, Up and Down move to
 * the same point of the next or previous series, Home and End go to the ends of the series, and
 * Escape hides the tooltip. Each mark's name (its label and value) is what screen readers read
 * and what the tooltip shows on hover, focus and touch. The tooltip is hidden from screen readers
 * so nothing is read twice. Directions follow `dir="rtl"`.
 * @module charts
 */

/** The figure, its marks and its tooltip. */
function chart(el) {
  const figure = el.closest('[data-zeyn-chart]');
  return { figure, tip: figure?.querySelector('.z-chart-tip'), marks: [...(figure?.querySelectorAll('.z-chart-mark') ?? [])] };
}

/** Shows the tooltip next to `mark`, kept inside the figure. */
function show(mark) {
  const { figure, tip } = chart(mark);
  if (!tip) return;
  tip.textContent = mark.getAttribute('data-zeyn-tip') ?? '';
  tip.hidden = false;
  const box = figure.getBoundingClientRect();
  const m = mark.getBoundingClientRect();
  const left = Math.min(Math.max(m.left + m.width / 2 - box.left - tip.offsetWidth / 2, 0), box.width - tip.offsetWidth);
  const top = Math.max(m.top - box.top - tip.offsetHeight - 8, 0);
  tip.style.left = `${left}px`;
  tip.style.top = `${top}px`;
}

/** Hides the tooltip of the chart holding `el`. */
function hide(el) {
  const { tip } = chart(el);
  if (tip) tip.hidden = true;
}

/** Moves the chart's tab stop to `next` and focuses it. */
function move(from, next) {
  if (!next || next === from) return;
  from.tabIndex = -1;
  next.tabIndex = 0;
  next.focus();
}

document.addEventListener('keydown', (e) => {
  const mark = e.target.closest?.('.z-chart-mark');
  if (!mark) return;
  if (e.key === 'Escape') {
    hide(mark);
    return;
  }
  const { marks } = chart(mark);
  const s = Number(mark.dataset.s);
  const i = Number(mark.dataset.i);
  const rtl = mark.closest('[dir="rtl"]') !== null;
  const same = marks.filter((m) => Number(m.dataset.s) === s);
  const at = same.indexOf(mark);
  const series = [...new Set(marks.map((m) => Number(m.dataset.s)))].sort((a, b) => a - b);
  const other = (d) => {
    const k = series.indexOf(s) + d;
    if (k < 0 || k >= series.length) return null;
    const list = marks.filter((m) => Number(m.dataset.s) === series[k]);
    return list.find((m) => Number(m.dataset.i) === i) ?? list[Math.min(at, list.length - 1)];
  };
  const keys = {
    ArrowRight: () => same[at + (rtl ? -1 : 1)],
    ArrowLeft: () => same[at + (rtl ? 1 : -1)],
    ArrowDown: () => other(1),
    ArrowUp: () => other(-1),
    Home: () => same[0],
    End: () => same[same.length - 1],
  };
  if (!(e.key in keys)) return;
  e.preventDefault();
  move(mark, keys[e.key]());
});

document.addEventListener('focusin', (e) => {
  const mark = e.target.closest?.('.z-chart-mark');
  if (mark) show(mark);
});

document.addEventListener('focusout', (e) => {
  const mark = e.target.closest?.('.z-chart-mark');
  if (mark && !e.relatedTarget?.closest?.('.z-chart-mark')) hide(mark);
});

document.addEventListener('pointerover', (e) => {
  const mark = e.target.closest?.('.z-chart-mark');
  if (mark) show(mark);
});

document.addEventListener('pointerout', (e) => {
  const mark = e.target.closest?.('.z-chart-mark');
  if (mark && document.activeElement !== mark) hide(mark);
});

document.addEventListener('pointerdown', (e) => {
  const mark = e.target.closest?.('.z-chart-mark');
  if (!mark) return;
  const { marks } = chart(mark);
  marks.forEach((m) => (m.tabIndex = m === mark ? 0 : -1));
  mark.focus();
});
