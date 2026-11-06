/**
 * Catalog runtime: card grids, rails and search boxes (`zeyn::ui::CardGrid`, `Rail`,
 * `SearchBox`). components.js loads it the first time a page holds one of them.
 *
 * Card grids and rails ask `POST /_zeyn/grid` for more cards with a position and a sort order
 * only; the server sets the page size and the search text. Search boxes ask `POST /_zeyn/data`
 * for suggestions and follow the W3C APG combobox pattern with a list box popup. A mouse can drag
 * a rail sideways; touch keeps native scrolling.
 * @module catalog
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const reduced = matchMedia('(prefers-reduced-motion: reduce)');

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/** Posts JSON with the CSRF header. Resolves to the parsed body, or `null` on any failure. */
async function post(path, body, signal) {
  try {
    const res = await fetch(BASE + path, {
      method: 'POST',
      credentials: 'same-origin',
      headers: {
        'content-type': 'application/json',
        'x-zeyn-csrf': await csrf(),
      },
      body: JSON.stringify(body),
      signal,
    });
    return res.ok ? await res.json() : null;
  } catch {
    return null;
  }
}

/** Screen name, params and node path of an element, as the endpoints expect them. */
function address(el) {
  const screen = el.closest('[data-zeyn-screen]');
  let params = null;
  try {
    params = JSON.parse(screen?.dataset.zeynParams ?? 'null');
  } catch {}
  return { screen: screen?.dataset.zeynScreen ?? null, params, node: el.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null };
}

/** Replaces `{name}` placeholders. */
function fill(template, values) {
  return (template ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in values ? String(values[k]) : m));
}

/* Card grids and rails ----------------------------------------------------------------------- */

/** The chosen sort order of a card grid, or none for the server's default. */
function sortOf(section) {
  const option = section.querySelector('[data-zeyn-cards-sort]')?.selectedOptions[0];
  try {
    return option ? [JSON.parse(option.dataset.sort)] : [];
  } catch {
    return [];
  }
}

/**
 * Shows the page position of a paged card grid and which of its buttons work. Without a total
 * from the action the label leaves out the page count and Next works while there is a cursor.
 */
function pager(section) {
  const d = section.dataset;
  const size = Number(d.zeynSize) || 1;
  const offset = Number(d.zeynOffset) || 0;
  const known = d.zeynTotal != null;
  const total = Number(d.zeynTotal) || 0;
  const page = Math.floor(offset / size) + 1;
  const label = section.querySelector('[data-zeyn-cards-page]');
  if (label) label.textContent = known ? fill(d.textPage, { page, pages: Math.max(1, Math.ceil(total / size)) }) : fill(d.textPageOnly, { page });
  const last = known ? offset + size >= total : !d.zeynNext;
  section.querySelector('[data-zeyn-cards-prev]')?.setAttribute('aria-disabled', String(offset <= 0));
  section.querySelector('[data-zeyn-cards-next]')?.setAttribute('aria-disabled', String(last));
}

/**
 * Loads cards: the next batch (`more`), the page at `offset`, or the first page again after the
 * sort changed. New cards are appended or replace the old ones; focus moves to the first new card
 * when the user asked with a button.
 */
async function load(section, how, focus) {
  const d = section.dataset;
  if (d.zeynLoading) return;
  const paged = d.zeynPaging === 'pages';
  const size = Number(d.zeynSize) || 0;
  let page;
  if (paged) {
    const offset = how === 'first' ? 0 : Math.max(0, (Number(d.zeynOffset) || 0) + (how === 'prev' ? -size : size));
    page = { type: 'offset', offset, limit: size };
  } else {
    if (how === 'more' && !d.zeynNext) return;
    page = { type: 'after', cursor: how === 'more' ? d.zeynNext : null, limit: size };
  }
  d.zeynLoading = '1';
  section.setAttribute('aria-busy', 'true');
  const body = await post('/_zeyn/grid', { ...address(section), query: { page, sort: sortOf(section), with_total: paged } });
  delete d.zeynLoading;
  section.removeAttribute('aria-busy');
  const status = section.querySelector('[data-zeyn-cards-status]');
  if (!body) {
    if (status) status.textContent = d.textError ?? '';
    return;
  }
  const list = section.querySelector('[data-zeyn-rows]');
  const before = how === 'more' ? list.children.length : 0;
  if (how === 'more') list.insertAdjacentHTML('beforeend', body.rows ?? '');
  else list.innerHTML = body.rows ?? '';
  if (body.next) d.zeynNext = body.next;
  else delete d.zeynNext;
  d.zeynOffset = String(body.offset ?? 0);
  if (body.total != null) d.zeynTotal = String(body.total);
  else delete d.zeynTotal;
  section.querySelector('[data-zeyn-cards-more]')?.toggleAttribute('hidden', !body.next);
  section.querySelector('[data-zeyn-cards-empty]')?.toggleAttribute('hidden', list.children.length > 0);
  if (paged) pager(section);
  if (status) status.textContent = fill(d.textLoaded, { count: list.children.length });
  document.dispatchEvent(new CustomEvent('zeyn:content', { detail: list }));
  if (focus) list.children[before]?.querySelector('a')?.focus();
  if (section.classList.contains('z-rail')) railButtons(section);
}

/** Whether a rail is scrolled to its start and to its end. */
function railEnds(list) {
  const left = Math.abs(list.scrollLeft);
  return { start: left <= 1, end: left + list.clientWidth >= list.scrollWidth - 1 };
}

/** Marks the rail's buttons that cannot scroll further, and loads more near the end. */
function railButtons(section) {
  const list = section.querySelector('[data-zeyn-rows]');
  if (!list) return;
  const { start, end } = railEnds(list);
  section.querySelector('[data-zeyn-rail-prev]')?.setAttribute('aria-disabled', String(start));
  section.querySelector('[data-zeyn-rail-next]')?.setAttribute('aria-disabled', String(end && !section.dataset.zeynNext));
  const left = list.scrollWidth - Math.abs(list.scrollLeft) - list.clientWidth;
  if (section.dataset.zeynNext && left < list.clientWidth) load(section, 'more', false);
}

/** Scrolls a rail by most of its width; `step` is 1 (forward) or -1. */
function railScroll(section, step) {
  const list = section.querySelector('[data-zeyn-rows]');
  if (!list) return;
  const rtl = getComputedStyle(list).direction === 'rtl' ? -1 : 1;
  list.scrollBy({ left: step * rtl * list.clientWidth * 0.9, behavior: reduced.matches ? 'auto' : 'smooth' });
}

/** Pixels a mouse moves before a press on a rail becomes a drag instead of a click. */
const DRAG_PX = 5;

/** Scrolls a rail so the card whose start is nearest to the rail's start lines up with it. */
function railSnap(list) {
  const rtl = getComputedStyle(list).direction === 'rtl';
  const box = list.getBoundingClientRect();
  const pad = parseFloat(getComputedStyle(list).scrollPaddingInlineStart) || 0;
  let best = null;
  for (const card of list.children) {
    const r = card.getBoundingClientRect();
    const delta = rtl ? r.right - (box.right - pad) : r.left - (box.left + pad);
    if (best === null || Math.abs(delta) < Math.abs(best)) best = delta;
  }
  if (best) list.scrollBy({ left: best, behavior: reduced.matches ? 'auto' : 'smooth' });
}

/**
 * Lets a mouse drag a rail sideways. Touch and pen keep the browser's own scrolling. A press that
 * moves more than `DRAG_PX` is a drag: the click on release is cancelled, so the card link does
 * not open, and the rail snaps to the nearest card. A smaller move is a normal click. The
 * dragging class stays until the mouse moves again, so the pointer events that follow the release
 * do not start a card preview.
 */
function railDrag(list) {
  let press = null;
  let dragged = false;
  list.addEventListener('pointerdown', (e) => {
    if (e.pointerType !== 'mouse' || e.button !== 0) return;
    press = { id: e.pointerId, x: e.clientX, left: list.scrollLeft, moving: false };
    dragged = false;
  });
  list.addEventListener('pointermove', (e) => {
    if (!press || e.pointerId !== press.id) return;
    const dx = e.clientX - press.x;
    if (!press.moving) {
      if (Math.abs(dx) <= DRAG_PX) return;
      press.moving = true;
      list.setPointerCapture(e.pointerId);
      list.classList.add('z-rail-dragging');
      getSelection()?.removeAllRanges();
    }
    list.scrollLeft = press.left - dx;
  });
  const release = (e) => {
    if (!press || e.pointerId !== press.id) return;
    const moving = press.moving;
    press = null;
    if (!moving) return;
    dragged = true;
    railSnap(list);
    list.addEventListener('pointermove', () => list.classList.remove('z-rail-dragging'), { once: true });
  };
  list.addEventListener('pointerup', release);
  list.addEventListener('pointercancel', release);
  list.addEventListener(
    'click',
    (e) => {
      if (!dragged) return;
      dragged = false;
      e.preventDefault();
      e.stopPropagation();
    },
    true,
  );
  list.addEventListener('dragstart', (e) => e.preventDefault());
  list.addEventListener('focusin', (e) => {
    e.target.closest?.('.z-poster')?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: reduced.matches ? 'auto' : 'smooth' });
  });
}

/** Whether the user scrolled the page; more cards load by themselves only after that. */
let scrolled = false;

const watcher = 'IntersectionObserver' in window
  ? new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (scrolled && entry.isIntersecting) load(entry.target.closest('[data-zeyn-cards]'), 'more', false);
        }
      },
      { rootMargin: '400px 0px' },
    )
  : null;

/**
 * The first scroll looks at every "Show more" button again. Loading cards while the page is
 * still being read would move the content under them, a layout shift the user did not cause.
 */
document.addEventListener(
  'scroll',
  () => {
    scrolled = true;
    for (const more of document.querySelectorAll('[data-zeyn-cards-more]')) {
      watcher?.unobserve(more);
      watcher?.observe(more);
    }
  },
  { passive: true, once: true, capture: true },
);

/** Sets up the card grids and rails inside `root`. */
function initCards(root) {
  for (const section of root.querySelectorAll?.('[data-zeyn-cards]:not([data-zeyn-ready])') ?? []) {
    section.dataset.zeynReady = '';
    if (section.dataset.zeynPaging === 'pages') pager(section);
    const more = section.querySelector('[data-zeyn-cards-more]');
    if (more) watcher?.observe(more);
    if (section.classList.contains('z-rail')) {
      const list = section.querySelector('[data-zeyn-rows]');
      let frame = 0;
      list?.addEventListener(
        'scroll',
        () => {
          if (frame) return;
          frame = requestAnimationFrame(() => {
            frame = 0;
            railButtons(section);
          });
        },
        { passive: true },
      );
      if (list) railDrag(list);
      railButtons(section);
    }
  }
}

document.addEventListener('click', (e) => {
  const button = e.target.closest?.('[data-zeyn-cards-more], [data-zeyn-cards-prev], [data-zeyn-cards-next], [data-zeyn-rail-prev], [data-zeyn-rail-next]');
  if (!button || button.getAttribute('aria-disabled') === 'true') return;
  const section = button.closest('[data-zeyn-cards]');
  if (button.matches('[data-zeyn-rail-prev]')) railScroll(section, -1);
  else if (button.matches('[data-zeyn-rail-next]')) railScroll(section, 1);
  else if (button.matches('[data-zeyn-cards-more]')) load(section, 'more', true);
  else load(section, button.matches('[data-zeyn-cards-prev]') ? 'prev' : 'next', true);
});

document.addEventListener('change', (e) => {
  if (e.target.matches?.('[data-zeyn-cards-sort]')) load(e.target.closest('[data-zeyn-cards]'), 'first', false);
});

/* Search boxes ------------------------------------------------------------------------------- */

const timers = new WeakMap();
const requests = new WeakMap();

/** Parts of a search box. */
function box(el) {
  const form = el.closest('[data-zeyn-searchbox]');
  return { form, input: form?.querySelector('[role="combobox"]'), list: form?.querySelector('[role="listbox"]') };
}

/** Closes the suggestions. */
function closeList(form) {
  const { input, list } = box(form);
  if (!list) return;
  list.hidden = true;
  input.setAttribute('aria-expanded', 'false');
  input.removeAttribute('aria-activedescendant');
}

/** Opens the suggestions when there are any. */
function openList(form) {
  const { input, list } = box(form);
  if (!list?.children.length) return false;
  list.hidden = false;
  input.setAttribute('aria-expanded', 'true');
  return true;
}

/** Highlights suggestion `index` (or none for -1). */
function highlight(form, index) {
  const { input, list } = box(form);
  const options = [...list.children];
  options.forEach((o, i) => o.setAttribute('aria-selected', String(i === index)));
  const active = options[index];
  if (active) {
    input.setAttribute('aria-activedescendant', active.id);
    active.scrollIntoView({ block: 'nearest' });
  } else input.removeAttribute('aria-activedescendant');
}

/** Asks the server for suggestions for the field's text and shows them. */
async function suggest(form) {
  const { input, list } = box(form);
  const text = input.value.trim().slice(0, Number(form.dataset.zeynMax) || 100);
  requests.get(form)?.abort();
  if (!text) {
    list.replaceChildren();
    closeList(form);
    return;
  }
  const controller = new AbortController();
  requests.set(form, controller);
  const hits = await post('/_zeyn/data', { ...address(form), key: 'suggest', input: text }, controller.signal);
  if (controller.signal.aborted || !Array.isArray(hits)) return;
  list.replaceChildren(
    ...hits.map((hit, i) => {
      if (hit && typeof hit === 'object') return richOption(hit, `${list.id}-${i}`);
      const li = document.createElement('li');
      li.id = `${list.id}-${i}`;
      li.setAttribute('role', 'option');
      li.setAttribute('aria-selected', 'false');
      li.className = 'z-search-option';
      li.textContent = String(hit);
      return li;
    }),
  );
  const status = form.querySelector('[data-zeyn-search-status]');
  if (status) status.textContent = hits.length ? fill(form.dataset.textCount, { count: hits.length }) : form.dataset.textNone ?? '';
  if (document.activeElement === input && !openList(form)) closeList(form);
}

/**
 * A rich suggestion of `SearchBox::item`: an option holding a real link (middle click and new
 * tab work) with an optional small image, the title and the subtitle. The option's name is its
 * text, the title and the subtitle; the image has an empty alternative text.
 * @param {{text: string, subtitle?: string, image?: string, href?: string}} hit
 */
function richOption(hit, id) {
  const li = document.createElement('li');
  li.id = id;
  li.setAttribute('role', 'option');
  li.setAttribute('aria-selected', 'false');
  li.className = 'z-search-option z-search-rich';
  const a = document.createElement(hit.href ? 'a' : 'span');
  a.className = 'z-search-rich-row';
  if (hit.href) {
    a.href = BASE + hit.href;
    a.tabIndex = -1;
  }
  if (hit.image) {
    const img = document.createElement('img');
    img.className = 'z-search-rich-img';
    img.src = hit.image;
    img.alt = '';
    img.width = 40;
    img.height = 56;
    img.loading = 'lazy';
    img.decoding = 'async';
    a.append(img);
  }
  const body = document.createElement('span');
  body.className = 'z-search-rich-body';
  const title = document.createElement('span');
  title.className = 'z-search-rich-title';
  title.textContent = String(hit.text ?? '');
  body.append(title);
  if (hit.subtitle) {
    const sub = document.createElement('span');
    sub.className = 'z-search-rich-sub';
    sub.textContent = ` ${hit.subtitle}`;
    body.append(sub);
  }
  a.append(body);
  li.append(a);
  return li;
}

/** Follows the link of a rich suggestion through `nav.js`; false when the option has none. */
function followOption(form, option) {
  const a = option?.querySelector('a[href]');
  if (!a) return false;
  closeList(form);
  const event = new CustomEvent('zeyn:navigate', { detail: { url: a.href }, cancelable: true });
  if (document.dispatchEvent(event)) location.assign(a.href);
  return true;
}

/** Opens the results screen with `text`, without a page reload. */
function search(form, text) {
  const { input } = box(form);
  const q = text.trim().slice(0, Number(form.dataset.zeynMax) || 100);
  input.value = q;
  closeList(form);
  if (form.dataset.zeynDrives) input.dispatchEvent(new Event('change', { bubbles: true }));
  const screen = form.dataset.zeynResults;
  if (!screen || !q) return;
  const url = `${BASE}/_zeyn/route/${encodeURIComponent(screen)}?params=${encodeURIComponent(JSON.stringify({ q }))}`;
  const event = new CustomEvent('zeyn:navigate', { detail: { url }, cancelable: true });
  if (document.dispatchEvent(event)) location.assign(url);
}

document.addEventListener('input', (e) => {
  if (!e.target.matches?.('[data-zeyn-searchbox] [role="combobox"]')) return;
  const { form } = box(e.target);
  clearTimeout(timers.get(form));
  timers.set(form, setTimeout(() => suggest(form), Number(form.dataset.zeynDelay) || 150));
});

document.addEventListener('keydown', (e) => {
  if (!e.target.matches?.('[data-zeyn-searchbox] [role="combobox"]')) return;
  const { form, input, list } = box(e.target);
  const options = [...list.children];
  const at = options.findIndex((o) => o.getAttribute('aria-selected') === 'true');
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    if (list.hidden) {
      if (openList(form)) highlight(form, e.key === 'ArrowDown' ? 0 : options.length - 1);
      return;
    }
    let next = at + (e.key === 'ArrowDown' ? 1 : -1);
    if (next >= options.length) next = -1;
    else if (next < -1) next = options.length - 1;
    highlight(form, next);
  } else if (e.key === 'Enter') {
    e.preventDefault();
    if (!list.hidden && at >= 0 && followOption(form, options[at])) return;
    search(form, !list.hidden && at >= 0 ? options[at].textContent : input.value);
  } else if (e.key === 'Escape') {
    if (!list.hidden) {
      e.preventDefault();
      closeList(form);
    } else if (input.value) {
      e.preventDefault();
      input.value = '';
      list.replaceChildren();
      if (form.dataset.zeynDrives) input.dispatchEvent(new Event('change', { bubbles: true }));
    }
  }
});

document.addEventListener('mousedown', (e) => {
  if (e.target.closest?.('.z-search-option')) e.preventDefault();
});

document.addEventListener('click', (e) => {
  const option = e.target.closest?.('.z-search-option');
  if (option?.querySelector('a[href]')) {
    if (!e.target.closest('a[href]')) followOption(option.closest('[data-zeyn-searchbox]'), option);
    else closeList(option.closest('[data-zeyn-searchbox]'));
    return;
  }
  if (option) search(option.closest('[data-zeyn-searchbox]'), option.textContent);
});

document.addEventListener('focusout', (e) => {
  const form = e.target.closest?.('[data-zeyn-searchbox]');
  if (form && !form.contains(e.relatedTarget)) closeList(form);
});

document.addEventListener('submit', (e) => {
  if (!e.target.matches?.('[data-zeyn-searchbox]')) return;
  e.preventDefault();
  search(e.target, box(e.target).input.value);
});

document.addEventListener('zeyn:content', (e) => initCards(e.detail instanceof Element ? e.detail : document));
initCards(document);
