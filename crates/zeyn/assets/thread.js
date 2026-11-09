/**
 * Message threads (`zeyn::ui::MessageThread`). Loaded only by pages that hold
 * `[data-zeyn-thread]`.
 *
 * History: the thread shows a window of at most `MAX_KEPT` messages. Scrolling near the top loads
 * the next older page through `POST /_zeyn/grid` (offsets count from the newest message), near
 * the bottom the newer page that was dropped; the far end of the window is dropped, and the
 * message the reader looks at keeps its place on screen (scroll anchoring by hand, so it works
 * in every browser). Messages carry their key, so a message that loads twice shows once. Day
 * headings and the grouping by sender are fixed where two pages meet.
 *
 * New messages: when a topic of the thread is named (`zeyn:topics`: a composer that sent, an
 * effect, the realtime hub) or a composer of the same conversation sent (`zeyn:chat-sent`), the
 * newest page is fetched and unseen messages are added at the bottom and announced in the polite
 * live region. A thread with a reply action then streams the AI reply from `POST /_zeyn/stream`
 * (`stream.js`): pieces append to one new message, only its last paragraph is drawn again, the
 * live region speaks the reply once it is finished, never every piece.
 *
 * Keys (APG feed): Page Down and Page Up move between messages, Home and End to the first and
 * last loaded one.
 * @module thread
 */

const S = await import(new URL(`stream.js${new URL(import.meta.url).search}`, import.meta.url).href);
const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const MAX_KEPT = 200;
const EDGE = 240;
const GAP_MS = 5 * 60 * 1000;
const MAX_REPLY = 32000;
const SAY_CHARS = 300;
/** @type {WeakMap<Element, object>} State per thread. */
const threads = new WeakMap();

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
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

/** Posts JSON with the CSRF header. */
async function post(path, body, signal) {
  return fetch(BASE + path, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
    body: JSON.stringify(body),
    signal,
  });
}

/** Messages of a thread in the page, oldest first. */
function messages(s) {
  return [...s.view.querySelectorAll(':scope > article')];
}

/** Says `text` in the thread's live region. */
function say(s, text) {
  s.status.textContent = '';
  requestAnimationFrame(() => (s.status.textContent = text.length > SAY_CHARS ? `${text.slice(0, SAY_CHARS)}…` : text));
}

/** The day heading of a message, made from its data when it has none. */
function heading(article, show) {
  let h = article.querySelector(':scope > .z-thread-day');
  if (!show) {
    h?.remove();
    return;
  }
  if (h) return;
  h = document.createElement('h3');
  h.className = 'z-thread-day';
  const span = document.createElement('span');
  span.textContent = article.dataset.dayLabel ?? '';
  h.append(span);
  article.prepend(h);
}

/** Sets the day heading and the grouping of `b`, which comes right after `a` (or first). */
function join(a, b) {
  if (!b) return;
  const newDay = !a || a.dataset.day !== b.dataset.day;
  heading(b, newDay);
  const cont = !!a && !newDay && a.dataset.who === b.dataset.who && Math.abs(Number(b.dataset.at) - Number(a.dataset.at)) < GAP_MS && a.classList.contains('z-thread-own') === b.classList.contains('z-thread-own');
  b.classList.toggle('z-thread-cont', cont);
}

/** The first message in view and its distance from the top of the view, to keep it in place. */
function anchor(s) {
  const top = s.view.getBoundingClientRect().top;
  for (const article of messages(s)) {
    const m = article.querySelector('.z-thread-bubble') ?? article;
    const r = m.getBoundingClientRect();
    if (r.bottom > top) return { m, y: r.top - top };
  }
  return null;
}

/** Scrolls so the anchored message is where it was. */
function restore(s, a) {
  if (!a?.m.isConnected) return;
  const y = a.m.getBoundingClientRect().top - s.view.getBoundingClientRect().top;
  s.view.scrollTop += y - a.y;
}

/** Whether the reader is at the newest message. */
function atBottom(s) {
  return s.view.scrollHeight - s.view.scrollTop - s.view.clientHeight < 80;
}

/** Messages of a page of rows, without those already shown. */
function fresh(s, html) {
  const t = document.createElement('template');
  t.innerHTML = html;
  return [...t.content.querySelectorAll('article')].filter((m) => !m.dataset.key || !s.keys.has(m.dataset.key));
}

/** Loads `limit` messages from `offset` (counted from the newest). `null` on failure. */
async function page(s, offset, limit) {
  try {
    const res = await post('/_zeyn/grid', { ...address(s.root), query: { page: { type: 'offset', offset, limit }, with_total: true } });
    if (!res.ok) return null;
    const body = await res.json();
    if (typeof body.total === 'number') s.total = body.total;
    return body;
  } catch {
    return null;
  }
}

/** Remembers the keys of added messages. */
function track(s, list) {
  for (const m of list) if (m.dataset.key) s.keys.add(m.dataset.key);
}

/** Forgets removed messages. */
function drop(s, list) {
  for (const m of list) {
    if (m.dataset.key) s.keys.delete(m.dataset.key);
    m.remove();
  }
}

/** Loads the next older page at the top and drops the newest beyond `MAX_KEPT`. */
async function older(s) {
  if (s.busy || s.hi >= s.total) return;
  s.busy = true;
  s.view.setAttribute('aria-busy', 'true');
  const body = await page(s, s.hi, s.size);
  s.busy = false;
  s.view.setAttribute('aria-busy', 'false');
  if (!body) return;
  const list = fresh(s, body.rows ?? '');
  s.hi += body.count ?? list.length;
  if (!list.length) return;
  const a = anchor(s);
  const first = messages(s)[0];
  s.view.prepend(...list);
  track(s, list);
  join(list[list.length - 1], first);
  heading(list[0], true);
  const all = messages(s);
  const extra = all.length - MAX_KEPT;
  if (extra > 0) {
    drop(s, all.slice(all.length - extra));
    s.lo += extra;
  }
  restore(s, a);
}

/** Loads the newer page that was dropped, at the bottom, and drops the oldest beyond `MAX_KEPT`. */
async function newer(s) {
  if (s.busy || s.lo <= 0) return;
  s.busy = true;
  const from = Math.max(0, s.lo - s.size);
  const body = await page(s, from, s.lo - from);
  s.busy = false;
  if (!body) return;
  const list = fresh(s, body.rows ?? '');
  s.lo = from;
  if (!list.length) return;
  const a = anchor(s);
  const all = messages(s);
  s.view.append(...list);
  track(s, list);
  join(all[all.length - 1], list[0]);
  const now = messages(s);
  const extra = now.length - MAX_KEPT;
  if (extra > 0) {
    drop(s, now.slice(0, extra));
    s.hi -= extra;
    heading(messages(s)[0], true);
  }
  restore(s, a);
}

/** Runs `latest` after the one still running, so a message never comes twice. */
function latest(s) {
  s.chain = s.chain.then(() => latestNow(s));
  return s.chain;
}

/** Fetches the newest page and adds the messages not shown yet; announces those of others. */
async function latestNow(s) {
  const before = s.total;
  if (s.lo > 0) {
    const body = await page(s, 0, 1);
    if (body) {
      s.lo += Math.max(0, s.total - before);
      s.hi += Math.max(0, s.total - before);
    }
    return;
  }
  const body = await page(s, 0, s.size);
  if (!body) return;
  const list = fresh(s, body.rows ?? '');
  if (!list.length) return;
  const stay = atBottom(s);
  const all = messages(s);
  s.view.querySelector(':scope > .z-thread-empty')?.remove();
  const streaming = s.view.querySelector(':scope > article[aria-busy="true"]');
  for (const m of list) s.view.insertBefore(m, streaming);
  track(s, list);
  join(all[all.length - 1], list[0]);
  s.hi += list.length;
  const extra = messages(s).length - MAX_KEPT;
  if (extra > 0 && stay) {
    drop(s, messages(s).slice(0, extra));
    s.hi -= extra;
    heading(messages(s)[0], true);
  }
  if (stay) s.view.scrollTop = s.view.scrollHeight;
  for (const m of list) {
    if (m.classList.contains('z-thread-own')) continue;
    const who = m.querySelector('.z-thread-who')?.textContent ?? '';
    const text = m.querySelector('.z-thread-text')?.textContent ?? '';
    say(s, s.t.new.replace('{name}', who).replace('{text}', text));
  }
}

/** Streams the AI reply to `text` into a new message at the bottom. */
async function reply(s, text) {
  if (s.reply) return;
  const m = document.createElement('article');
  m.className = 'z-thread-msg z-thread-other z-thread-ai';
  m.tabIndex = -1;
  m.dataset.day = s.t.todayDate ?? '';
  m.dataset.dayLabel = s.t.today ?? '';
  m.dataset.who = 'ai';
  m.dataset.at = String(Date.now());
  m.setAttribute('aria-busy', 'true');
  m.append(S.el('p', 'z-thread-who', s.t.assistant));
  const bubble = S.el('div', 'z-thread-bubble');
  const body = S.el('div', 'z-thread-text');
  body.dir = 'auto';
  const typing = S.el('span', 'z-thread-typing');
  typing.append(S.el('span', 'z-sr-only', s.t.writing));
  body.append(typing);
  const stop = S.el('button', 'z-button z-thread-stop', s.t.stop);
  stop.type = 'button';
  bubble.append(body, stop);
  m.append(bubble);
  const last = messages(s).at(-1);
  s.view.querySelector(':scope > .z-thread-empty')?.remove();
  s.view.append(m);
  join(last, m);
  s.view.scrollTop = s.view.scrollHeight;
  const ctl = new AbortController();
  s.reply = ctl;
  stop.addEventListener('click', () => ctl.abort());
  const pin = () => {
    if (s.pinned) s.view.scrollTop = s.view.scrollHeight;
  };
  s.pinned = true;
  const w = S.writer(body, MAX_REPLY, pin);
  let failed = false;
  try {
    const res = await post('/_zeyn/stream', { ...address(s.root), key: 'reply', input: { text } }, ctl.signal);
    if (!res.ok) failed = true;
    else {
      await S.read(res, (e) => {
        if (e.t === 'start' && typeof e.key === 'string') {
          if (s.keys.has(e.key)) s.view.querySelector(`:scope > article[data-key="${CSS.escape(e.key)}"]`)?.remove();
          m.dataset.key = e.key;
          s.keys.add(e.key);
        } else if (e.t === 'text') {
          typing.remove();
          w.push(e.v);
        } else if (e.t === 'error') failed = true;
      });
    }
  } catch (err) {
    if (err?.name === 'AbortError') {
      bubble.append(S.el('p', 'z-thread-note', s.t.stopped));
      say(s, s.t.stopped);
    } else failed = true;
  }
  typing.remove();
  w.end();
  stop.remove();
  m.removeAttribute('aria-busy');
  s.reply = null;
  if (failed) {
    bubble.append(S.el('p', 'z-thread-error', s.t.failed));
    say(s, s.t.failed);
  } else if (w.text()) {
    s.hi += 1;
    say(s, `${s.t.assistant}: ${body.textContent}`);
  }
  if (!w.text() && failed) m.querySelector('.z-thread-text')?.remove();
  pin();
}

/** Moves focus to the message `step` places from the focused one (APG feed). */
function move(s, from, step) {
  const all = messages(s);
  const i = all.indexOf(from);
  const next = i < 0 ? all[step > 0 ? 0 : all.length - 1] : all[Math.max(0, Math.min(all.length - 1, i + step))];
  next?.focus();
  if (next && all.indexOf(next) === 0) older(s);
  if (next && all.indexOf(next) === all.length - 1) newer(s);
}

/** Sets up the threads inside `root`. */
function init(root) {
  root.querySelectorAll?.('[data-zeyn-thread]:not([data-ready])').forEach((el) => {
    el.dataset.ready = '';
    const view = el.querySelector('.z-thread-view');
    if (!view) return;
    let t = {};
    try {
      t = JSON.parse(el.dataset.texts ?? '{}');
    } catch {}
    const s = {
      root: el,
      view,
      t,
      status: el.querySelector('[data-zeyn-thread-status]'),
      room: el.dataset.room ?? '',
      topics: (el.dataset.topics ?? '').split(',').filter(Boolean),
      size: Number(el.dataset.page) || 50,
      total: Number(el.dataset.total) || 0,
      lo: 0,
      hi: Number(el.dataset.count) || 0,
      keys: new Set(),
      busy: false,
      reply: null,
      pinned: true,
      frame: 0,
      chain: Promise.resolve(),
    };
    threads.set(el, s);
    track(s, messages(s));
    view.scrollTop = view.scrollHeight;
    view.addEventListener(
      'scroll',
      () => {
        s.pinned = atBottom(s);
        s.frame ||= requestAnimationFrame(() => {
          s.frame = 0;
          if (view.scrollTop < EDGE) older(s);
          else if (view.scrollHeight - view.scrollTop - view.clientHeight < EDGE) newer(s);
        });
      },
      { passive: true },
    );
    view.addEventListener('keydown', (e) => {
      const from = e.target instanceof Element ? e.target.closest('.z-thread-view > article') : null;
      if (e.target !== view && !from) return;
      const steps = { PageDown: 1, PageUp: -1, Home: -Infinity, End: Infinity };
      if (!(e.key in steps) || e.altKey || e.metaKey || e.ctrlKey) return;
      e.preventDefault();
      const step = steps[e.key];
      if (Number.isFinite(step)) move(s, from, step);
      else move(s, null, step > 0 ? -1 : 1);
    });
  });
}

document.addEventListener('zeyn:topics', (e) => {
  const named = new Set(e.detail?.topics ?? []);
  for (const el of document.querySelectorAll('[data-zeyn-thread]')) {
    const s = threads.get(el);
    if (s && s.topics.some((t) => named.has(t))) latest(s);
  }
});

document.addEventListener('zeyn:chat-sent', async (e) => {
  const room = e.detail?.room ?? '';
  const text = e.detail?.text ?? '';
  for (const el of document.querySelectorAll('[data-zeyn-thread]')) {
    const s = threads.get(el);
    if (!s || s.room !== room) continue;
    await latest(s);
    s.view.scrollTop = s.view.scrollHeight;
    if (el.hasAttribute('data-zeyn-thread-reply') && text) reply(s, text);
  }
});

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
