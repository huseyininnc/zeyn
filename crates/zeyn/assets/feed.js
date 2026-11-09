/**
 * Feeds (`zeyn::ui::Feed`): a `role="feed"` of posts that loads older posts near its end, holds
 * new posts behind a button, and keeps only the posts near the viewport in the page. Loaded only
 * by pages that hold `[data-zeyn-feed]`.
 *
 * Every post is an item `{key, el, html, h}`: in the page (`el`) or kept as HTML with the height
 * it had, measured when it left. The posts above and below the window are padding of their
 * summed height, so nothing moves when posts leave or come back. A focused post that leaves the
 * window stays in the page, placed over its own slot. Above `MAX_ITEMS` posts the oldest ones far
 * behind the reader are dropped and the button offers a way back to the newest.
 *
 * Older posts come from `POST /_zeyn/grid` with the cursor of the last batch; the server sets the
 * batch size. When a topic of the feed is named (`zeyn:topics`, or a `refresh` effect), the feed
 * asks for its first batch and holds the posts above its newest one; the button says how many.
 * Pressing it inserts them above with the scroll position kept, then focuses the first one.
 * Page Down and Page Up move focus between posts, Control+End and Control+Home leave the feed
 * (W3C APG feed pattern).
 * @module feed
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const MARGIN = 800;
const MAX_ITEMS = 1000;
const reduced = matchMedia('(prefers-reduced-motion: reduce)');
/** @type {WeakMap<Element, object>} State per feed section. */
const feeds = new WeakMap();
let frame = 0;
let scrolled = false;

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/** Asks the server for a batch of posts after `cursor` (the newest batch for `null`). */
async function batch(section, cursor) {
  const screen = section.closest('[data-zeyn-screen]');
  let params = null;
  try {
    params = JSON.parse(screen?.dataset.zeynParams ?? 'null');
  } catch {}
  try {
    const res = await fetch(BASE + '/_zeyn/grid', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify({
        screen: screen?.dataset.zeynScreen ?? null,
        params,
        node: section.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
        query: { page: { type: 'after', cursor, limit: 0 } },
      }),
    });
    return res.ok ? await res.json() : null;
  } catch {
    return null;
  }
}

/** The posts of a batch's HTML as elements. */
function parse(html) {
  const t = document.createElement('template');
  t.innerHTML = html ?? '';
  return [...t.content.children].filter((el) => el.matches('article[data-zeyn-key]'));
}

/** An element of an item, made again from its HTML when it is out of the page. */
function elementOf(item) {
  if (!item.el) {
    item.el = parse(item.html)[0] ?? document.createElement('article');
    item.html = null;
  }
  return item.el;
}

/** Replaces `{name}` placeholders. */
function fill(template, values) {
  return (template ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in values ? String(values[k]) : m));
}

/** Writes the place of every post in the page and the size of the set. */
function number(s) {
  const size = s.next ? '-1' : String(s.base + s.items.length);
  for (let i = s.from; i < s.to; i++) {
    const el = s.items[i].el;
    if (!el) continue;
    el.setAttribute('aria-posinset', String(s.base + i + 1));
    el.setAttribute('aria-setsize', size);
  }
  if (s.kept) s.kept.el.setAttribute('aria-setsize', size);
}

/** Sums the heights of items `a` to `b`. */
function span(s, a, b) {
  let h = 0;
  for (let i = a; i < b; i++) h += s.items[i].h;
  return h;
}

/**
 * Keeps the posts near the viewport in the page. Heights of the posts in the page are read
 * first, then the window is moved: posts that leave keep their HTML and height, posts that come
 * back are made from it, and the padding above and below stands for the rest.
 */
function paint(s) {
  const { list, items } = s;
  for (let i = s.from; i < s.to; i++) {
    const it = items[i];
    if (it.el && it !== s.kept) it.h = it.el.offsetHeight + s.gap;
  }
  const top = list.getBoundingClientRect().top;
  const start = -top - MARGIN;
  const end = -top + innerHeight + MARGIN;
  let y = 0;
  let from = items.length;
  let to = items.length;
  for (let i = 0; i < items.length; i++) {
    if (from === items.length && y + items[i].h > start) from = i;
    if (y >= end) {
      to = i;
      break;
    }
    y += items[i].h;
  }
  from = Math.min(from, to);
  if (to === items.length && y < end && scrolled) more(s);
  const active = document.activeElement?.closest?.('article[data-zeyn-key]');
  const focused = active && list.contains(active) ? items.find((it) => it.el === active) : null;
  const at = focused ? items.indexOf(focused) : -1;
  if (from === s.from && to === s.to && (at < 0 || s.kept === focused || (at >= from && at < to))) return;
  for (let i = s.from; i < s.to; i++) {
    const it = items[i];
    if ((i >= from && i < to) || !it.el) continue;
    if (it === focused) continue;
    it.html = it.el.outerHTML;
    it.el.remove();
    it.el = null;
  }
  if (s.kept && s.kept !== focused) {
    s.kept.el?.classList.remove('z-post-kept');
    s.kept.el?.style.removeProperty('top');
    const i = items.indexOf(s.kept);
    if ((i < from || i >= to) && s.kept.el) {
      s.kept.html = s.kept.el.outerHTML;
      s.kept.el.remove();
      s.kept.el = null;
    }
  }
  s.kept = null;
  let before = null;
  for (let i = to - 1; i >= from; i--) {
    const it = items[i];
    const el = elementOf(it);
    if (el.parentElement !== list || el.classList.contains('z-post-kept')) {
      el.classList.remove('z-post-kept');
      el.style.removeProperty('top');
      list.insertBefore(el, before);
    } else if (before && el.nextElementSibling !== before) {
      list.insertBefore(el, before);
    }
    before = el;
  }
  if (focused) {
    const i = items.indexOf(focused);
    if (i < from || i >= to) {
      focused.el.classList.add('z-post-kept');
      focused.el.style.top = `${span(s, 0, i)}px`;
      s.kept = focused;
    }
  }
  s.from = from;
  s.to = to;
  list.style.paddingBlockStart = `${span(s, 0, from)}px`;
  list.style.paddingBlockEnd = `${span(s, to, items.length)}px`;
  number(s);
  trim(s);
}

/** Drops the oldest posts far above the reader once the feed holds more than `MAX_ITEMS`. */
function trim(s) {
  const over = s.items.length - MAX_ITEMS;
  if (over <= 0 || s.from < over) return;
  const gone = span(s, 0, over);
  s.items.splice(0, over);
  s.base += over;
  s.from -= over;
  s.to -= over;
  s.list.style.paddingBlockStart = `${span(s, 0, s.from)}px`;
  scrollBy(0, -gone);
  s.trimmed = true;
  pill(s, s.texts.newest);
}

/** Shows the button over the feed with `text`, or hides it for an empty text. */
function pill(s, text) {
  s.pill.textContent = text ?? '';
  s.pill.hidden = !text;
}

/** Says `text` through the feed's status region. */
function say(s, text) {
  if (s.status) s.status.textContent = text;
}

/** Paints every feed of the page in the next frame. */
function schedule() {
  frame ||= requestAnimationFrame(() => {
    frame = 0;
    for (const section of document.querySelectorAll('[data-zeyn-feed]')) {
      const s = feeds.get(section);
      if (s && section.isConnected) paint(s);
    }
  });
}

/** Loads the next batch of older posts at the end of the feed. */
async function more(s) {
  if (s.loading || !s.next) return;
  s.loading = true;
  s.list.setAttribute('aria-busy', 'true');
  const body = await batch(s.section, s.next);
  s.loading = false;
  s.list.setAttribute('aria-busy', 'false');
  if (!body) {
    say(s, s.texts.error);
    return;
  }
  s.next = body.next ?? null;
  s.more.hidden = !s.next;
  const known = new Set(s.items.map((it) => it.key));
  const fresh = parse(body.rows).filter((el) => !known.has(el.dataset.zeynKey));
  const atEnd = s.to === s.items.length;
  const average = s.items.length ? span(s, 0, s.items.length) / s.items.length : 400;
  for (const el of fresh) {
    const item = { key: el.dataset.zeynKey, el: atEnd ? el : null, html: atEnd ? null : el.outerHTML, h: average };
    s.items.push(item);
    if (atEnd) s.list.append(el);
  }
  if (atEnd) s.to = s.items.length;
  number(s);
  if (atEnd && fresh.length) document.dispatchEvent(new CustomEvent('zeyn:content', { detail: s.list }));
  say(s, fill(s.texts.loaded, { count: s.base + s.items.length }));
  schedule();
}

/** Looks for posts newer than the newest one the feed holds and offers them on the button. */
async function check(s) {
  if (s.checking) {
    s.again = true;
    return;
  }
  s.checking = true;
  const body = await batch(s.section, null);
  s.checking = false;
  if (body) {
    const rows = parse(body.rows);
    const at = rows.findIndex((el) => el.dataset.zeynKey === s.newest);
    s.held = at < 0 ? rows : rows.slice(0, at);
    s.split = at < 0;
    s.gapNext = body.next ?? null;
    const n = s.held.length;
    if (n) {
      const text = s.split ? fill(s.texts.new_more, { count: n }) : n === 1 ? s.texts.new_one : fill(s.texts.new_many, { count: n });
      pill(s, text);
      say(s, text);
    }
  }
  if (s.again) {
    s.again = false;
    check(s);
  }
}

/** Starts the feed again from its newest batch, as the button does after a gap or a trim. */
function restart(s, rows, next) {
  for (const it of s.items) it.el?.remove();
  s.items = rows.map((el) => ({ key: el.dataset.zeynKey, el, html: null, h: 400 }));
  s.list.append(...rows);
  s.from = 0;
  s.to = s.items.length;
  s.base = 0;
  s.kept = null;
  s.trimmed = false;
  s.next = next;
  s.more.hidden = !next;
  s.newest = s.items[0]?.key ?? null;
  s.list.style.paddingBlockStart = s.list.style.paddingBlockEnd = '0px';
  number(s);
}

/**
 * The button: inserts the held new posts above the newest one with the scroll position kept,
 * then focuses the first of them; after a gap or a trim it starts the feed from the newest
 * batch.
 */
async function showNew(s) {
  pill(s, '');
  let rows = s.held;
  s.held = [];
  if (s.trimmed && !rows.length) {
    const body = await batch(s.section, null);
    if (!body) return say(s, s.texts.error);
    rows = parse(body.rows);
    s.split = true;
    s.gapNext = body.next ?? null;
  }
  if (!rows.length) return;
  const behavior = reduced.matches ? 'auto' : 'smooth';
  if (s.split || s.trimmed) {
    restart(s, rows, s.gapNext);
  } else {
    if (s.from > 0) {
      scrollTo({ top: scrollY + s.section.getBoundingClientRect().top, behavior: 'auto' });
      paint(s);
    }
    const anchor = s.items[s.from]?.el;
    const before = anchor?.getBoundingClientRect().top ?? 0;
    for (let i = rows.length - 1; i >= 0; i--) s.list.insertBefore(rows[i], s.list.firstElementChild);
    s.items.unshift(...rows.map((el) => ({ key: el.dataset.zeynKey, el, html: null, h: 400 })));
    s.to += rows.length;
    if (anchor) scrollBy(0, anchor.getBoundingClientRect().top - before);
    s.newest = s.items[0].key;
    number(s);
  }
  document.dispatchEvent(new CustomEvent('zeyn:content', { detail: s.list }));
  const first = s.items[0]?.el;
  first?.focus({ preventScroll: true });
  first?.scrollIntoView({ block: 'start', behavior });
  schedule();
}

/** The focusable elements of the page in tab order, for leaving the feed. */
function focusables() {
  return [...document.querySelectorAll('a[href], button:not([disabled]), input:not([disabled]), select, textarea, [tabindex]:not([tabindex="-1"])')].filter(
    (el) => !el.hidden && el.offsetParent !== null && !el.closest('[inert]'),
  );
}

/** Page Down and Page Up between posts, Control+End and Control+Home out of the feed. */
function keys(s, e) {
  const post = e.target.closest?.('article[data-zeyn-key]');
  if (!post || !s.list.contains(post)) return;
  if (e.key === 'PageDown' || e.key === 'PageUp') {
    e.preventDefault();
    const i = s.items.findIndex((it) => it.el === post);
    const next = s.items[i + (e.key === 'PageDown' ? 1 : -1)];
    if (!next) {
      if (e.key === 'PageDown') more(s);
      return;
    }
    const el = elementOf(next);
    if (!el.isConnected) paint(s);
    el.focus({ preventScroll: true });
    el.scrollIntoView({ block: 'nearest', behavior: reduced.matches ? 'auto' : 'smooth' });
    schedule();
  } else if ((e.key === 'End' || e.key === 'Home') && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    const all = focusables();
    const target = e.key === 'End'
      ? all.find((el) => !s.section.contains(el) && s.section.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING)
      : all.reverse().find((el) => !s.section.contains(el) && s.section.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_PRECEDING);
    target?.focus();
  }
}

const watcher = 'IntersectionObserver' in window
  ? new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const s = feeds.get(entry.target.closest('[data-zeyn-feed]'));
          if (s && scrolled && entry.isIntersecting) more(s);
        }
      },
      { rootMargin: `${MARGIN}px 0px` },
    )
  : null;

/** Sets up the feeds inside `root`. */
function init(root) {
  for (const section of root.querySelectorAll?.('[data-zeyn-feed]') ?? []) {
    if (feeds.has(section)) continue;
    const list = section.querySelector('[data-zeyn-rows]');
    const more_ = section.querySelector('[data-zeyn-feed-more]');
    let texts = {};
    try {
      texts = JSON.parse(section.dataset.texts ?? '{}') ?? {};
    } catch {}
    const items = [...list.children].map((el) => ({ key: el.dataset.zeynKey, el, html: null, h: 0 }));
    const s = {
      section, list, items, texts, more: more_, pill: section.querySelector('[data-zeyn-feed-pill]'),
      status: section.querySelector('[data-zeyn-feed-status]'), next: section.dataset.zeynNext ?? null,
      from: 0, to: items.length, base: 0, kept: null, held: [], split: false, gapNext: null, trimmed: false, gap: 0,
      newest: items[0]?.key ?? null, topics: (section.dataset.zeynFeed ?? '').split(',').filter(Boolean),
    };
    if (items[0]) s.gap = parseFloat(getComputedStyle(items[0].el).marginBlockEnd) || 0;
    feeds.set(section, s);
    more_?.addEventListener('click', () => more(s));
    s.pill?.addEventListener('click', () => showNew(s));
    list.addEventListener('keydown', (e) => keys(s, e));
    list.addEventListener('focusin', schedule);
    if (more_) watcher?.observe(more_);
    number(s);
    schedule();
  }
}

/** True when one of the feed's topics is in `topics`. */
function named(s, topics) {
  return Array.isArray(topics) && s.topics.some((t) => topics.includes(t));
}

/** Checks every feed of the page that listens to one of `topics`. */
function topics(list) {
  for (const section of document.querySelectorAll('[data-zeyn-feed]')) {
    const s = feeds.get(section);
    if (s && named(s, list)) check(s);
  }
}

addEventListener('scroll', () => {
  scrolled = true;
  schedule();
}, { passive: true });
addEventListener('resize', schedule, { passive: true });
document.addEventListener('zeyn:topics', (e) => topics(e.detail?.topics));
document.addEventListener('zeyn:effect', (e) => {
  const effect = e.detail?.effect;
  if (effect?.effect === 'refresh') topics(effect.topics);
});
document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
