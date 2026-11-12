/**
 * Picture comparisons (`zeyn::ui::ImageCompare`). Loaded only by pages that hold
 * `[data-zeyn-imgcompare]`. It shows the slider, which the server renders hidden, and writes its
 * value to `--z-imgcompare` so the stylesheet uncovers that share of the first picture.
 * @module imgcompare
 */

/** Shows the sliders under `root` and sets their starting share. */
function setup(root) {
  const found = root.matches?.('[data-zeyn-imgcompare]') ? [root] : [...(root.querySelectorAll?.('[data-zeyn-imgcompare]') ?? [])];
  for (const box of found) {
    const range = box.querySelector('.z-imgcompare-range');
    if (!range || !range.hidden) continue;
    range.hidden = false;
    box.style.setProperty('--z-imgcompare', `${range.value}%`);
  }
}

document.addEventListener('input', (e) => {
  const range = e.target instanceof Element ? e.target.closest('.z-imgcompare-range') : null;
  range?.closest('[data-zeyn-imgcompare]')?.style.setProperty('--z-imgcompare', `${range.value}%`);
});

document.addEventListener('zeyn:content', (e) => setup(e.detail ?? document));
setup(document);
