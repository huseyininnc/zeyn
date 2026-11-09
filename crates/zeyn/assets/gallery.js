/**
 * Product galleries (`zeyn::ui::ProductGallery`). Loaded only by pages that hold
 * `[data-zeyn-gallery]`. The slides sit in a scroll snapping track, so phones swipe between
 * them without script. This module watches which slide is on screen with one
 * IntersectionObserver per gallery (the scroll position is read only when a slide crosses the
 * threshold, never per scroll frame), puts in the images of that
 * slide and its two neighbors from their templates, marks the current thumbnail, moves with the
 * thumbnails and the arrow keys, zooms the image under a fine pointer (the slide's box is read
 * once when the pointer enters) and opens a full screen view: a modal `<dialog>` with the image,
 * previous and next buttons and a close button. The dialog keeps focus inside, closes with
 * Escape and gives focus back to what opened it.
 * @module gallery
 */

const reduce = matchMedia('(prefers-reduced-motion: reduce)');

/** The slides of a gallery. */
function slides(gallery) {
  return [...gallery.querySelectorAll('.z-gallery-slide')];
}

/** Puts in the image of slide `i` when it still waits in its template. */
function fill(gallery, i) {
  const slide = slides(gallery)[i];
  const t = slide?.querySelector(':scope > template');
  if (t) t.replaceWith(t.content.cloneNode(true));
}

/** Makes slide `i` the current one: neighbors loaded, thumbnail marked. */
function current(gallery, i) {
  gallery.zeynIndex = i;
  for (const n of [i - 1, i, i + 1]) fill(gallery, n);
  for (const b of gallery.querySelectorAll('.z-gallery-thumb')) {
    if (Number(b.dataset.index) === i) b.setAttribute('aria-current', 'true');
    else b.removeAttribute('aria-current');
  }
}

/** Scrolls the track to slide `i`. */
function go(gallery, i) {
  const all = slides(gallery);
  const at = Math.max(0, Math.min(all.length - 1, i));
  fill(gallery, at);
  all[at]?.scrollIntoView({ behavior: reduce.matches ? 'auto' : 'smooth', block: 'nearest', inline: 'start' });
  current(gallery, at);
  if (gallery.zeynDialog?.open) show(gallery);
}

/** Watches the track so the slide on screen becomes the current one. */
function watch(gallery) {
  if (gallery.zeynWatched) return;
  gallery.zeynWatched = true;
  gallery.zeynIndex = 0;
  const track = gallery.querySelector('.z-gallery-track');
  if (!track || !('IntersectionObserver' in window)) return;
  const io = new IntersectionObserver(
    (entries) => {
      if (!entries.some((e) => e.isIntersecting)) return;
      const at = Math.round(Math.abs(track.scrollLeft) / (track.clientWidth || 1));
      current(gallery, Math.min(slides(gallery).length - 1, at));
    },
    { root: track, threshold: 0.6 },
  );
  for (const s of slides(gallery)) io.observe(s);
  fill(gallery, 1);
}

/** Writes the current image into the full screen view. */
function show(gallery) {
  const d = gallery.zeynDialog;
  const i = gallery.zeynIndex ?? 0;
  fill(gallery, i);
  const slide = slides(gallery)[i];
  const img = slide?.querySelector('img.z-image') ?? slide?.querySelector('img');
  const big = d.querySelector('.z-gallery-full-img');
  big.src = img?.currentSrc || img?.src || '';
  big.alt = img?.alt ?? '';
  big.classList.remove('z-gallery-zoomed');
  d.querySelector('.z-gallery-full-count').textContent = slides(gallery)[i]?.getAttribute('aria-label') ?? '';
}

/** Builds the full screen view of a gallery once. */
function dialog(gallery) {
  if (gallery.zeynDialog) return gallery.zeynDialog;
  const d = document.createElement('dialog');
  d.className = 'z-gallery-full';
  d.setAttribute('aria-label', gallery.getAttribute('aria-label') ?? '');
  const button = (cls, text) => {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = cls;
    b.setAttribute('aria-label', text);
    return b;
  };
  const close = button('z-gallery-full-close', gallery.dataset.textClose ?? '');
  const prev = button('z-gallery-full-prev', gallery.dataset.textPrev ?? '');
  const next = button('z-gallery-full-next', gallery.dataset.textNext ?? '');
  const zoom = button('z-gallery-full-zoom', gallery.dataset.textZoom ?? '');
  zoom.setAttribute('aria-pressed', 'false');
  const img = document.createElement('img');
  img.className = 'z-gallery-full-img';
  img.decoding = 'async';
  const count = document.createElement('p');
  count.className = 'z-gallery-full-count';
  count.setAttribute('aria-live', 'polite');
  d.append(close, prev, img, next, zoom, count);
  close.addEventListener('click', () => d.close());
  prev.addEventListener('click', () => go(gallery, (gallery.zeynIndex ?? 0) - 1));
  next.addEventListener('click', () => go(gallery, (gallery.zeynIndex ?? 0) + 1));
  const toggle = () => {
    const on = img.classList.toggle('z-gallery-zoomed');
    zoom.setAttribute('aria-pressed', String(on));
  };
  zoom.addEventListener('click', toggle);
  img.addEventListener('click', toggle);
  d.addEventListener('close', () => gallery.zeynOpener?.focus({ preventScroll: true }));
  gallery.append(d);
  gallery.zeynDialog = d;
  return d;
}

/** Opens the full screen view on the current image. */
function open(gallery, opener) {
  const d = dialog(gallery);
  gallery.zeynOpener = opener;
  show(gallery);
  d.showModal();
  d.querySelector('.z-gallery-full-close').focus();
}

/** Sets up the galleries under `root`. */
function setup(root) {
  if (root instanceof Element && root.matches('[data-zeyn-gallery]')) watch(root);
  for (const g of root.querySelectorAll?.('[data-zeyn-gallery]') ?? []) watch(g);
}

setup(document);
document.addEventListener('zeyn:content', (e) => setup(e.detail ?? document));

document.addEventListener('click', (e) => {
  const target = e.target instanceof Element ? e.target : null;
  const gallery = target?.closest('[data-zeyn-gallery]');
  if (!gallery || target.closest('dialog')) return;
  const thumb = target.closest('.z-gallery-thumb');
  if (thumb) return go(gallery, Number(thumb.dataset.index));
  const opener = target.closest('.z-gallery-open');
  if (opener) return open(gallery, opener);
  if (target.closest('.z-gallery-slide img')) open(gallery, gallery.querySelector('.z-gallery-open'));
});

document.addEventListener('keydown', (e) => {
  const target = e.target instanceof Element ? e.target : null;
  const inside = target?.closest('.z-gallery-track, .z-gallery-full');
  if (!inside || (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight')) return;
  const gallery = inside.closest('[data-zeyn-gallery]');
  if (!gallery) return;
  e.preventDefault();
  const rtl = getComputedStyle(inside).direction === 'rtl';
  const forward = (e.key === 'ArrowRight') !== rtl;
  go(gallery, (gallery.zeynIndex ?? 0) + (forward ? 1 : -1));
});

document.addEventListener('pointerover', (e) => {
  if (e.pointerType !== 'mouse') return;
  const slide = e.target instanceof Element ? e.target.closest('[data-zeyn-gallery] .z-gallery-slide') : null;
  if (!slide || slide.zeynBox) return;
  slide.zeynBox = slide.getBoundingClientRect();
  slide.classList.add('z-gallery-zoom');
});

document.addEventListener('pointermove', (e) => {
  const slide = e.target instanceof Element ? e.target.closest('.z-gallery-slide.z-gallery-zoom') : null;
  const box = slide?.zeynBox;
  if (!box) return;
  slide.style.setProperty('--zx', `${(((e.clientX - box.left) * 100) / box.width).toFixed(1)}%`);
  slide.style.setProperty('--zy', `${(((e.clientY - box.top) * 100) / box.height).toFixed(1)}%`);
});

document.addEventListener('pointerout', (e) => {
  const slide = e.target instanceof Element ? e.target.closest('.z-gallery-slide.z-gallery-zoom') : null;
  if (!slide || slide.contains(e.relatedTarget)) return;
  slide.classList.remove('z-gallery-zoom');
  slide.zeynBox = null;
});
