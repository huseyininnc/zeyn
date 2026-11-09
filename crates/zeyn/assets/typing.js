/**
 * Typing indicators (`zeyn::ui::TypingIndicator`). Loaded only by pages that hold
 * `[data-zeyn-typing]`, which the server renders only when the app has a realtime channel.
 *
 * A composer fires `zeyn:typing` with its conversation while the user types (at most every 3
 * seconds); this module sends `{ t: "typing", topic, room }` on the page's live socket
 * (`live.js`), again at most every 3 seconds. `live.js` passes typing signals of other users
 * on as `zeyn:signal`; an indicator of the same topic and conversation shows the names for 6
 * seconds after the last signal, never the user's own. Names are set with `textContent`.
 * @module typing
 */

const EVERY_MS = 3000;
const SHOWN_MS = 6000;
/** @type {Map<string, number>} When a conversation last sent a signal, by `topic room`. */
const sent = new Map();
/** @type {WeakMap<Element, Map<string, number>>} Who is typing, by key, until when. */
const typing = new WeakMap();
const timers = new WeakMap();

/** Parses JSON, or returns `null`. */
function parse(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** Writes who is typing into an indicator and plans the next expiry. */
function draw(el) {
  const now = Date.now();
  const who = typing.get(el) ?? new Map();
  for (const [k, until] of who) if (until <= now) who.delete(k);
  el.zeynPeople ??= parse(el.dataset.people ?? '{}') ?? {};
  el.zeynTexts ??= parse(el.dataset.texts ?? '{}') ?? {};
  const t = el.zeynTexts;
  const names = [...who.keys()].map((k) => el.zeynPeople[k] ?? t.someone ?? '');
  const text = names.length === 0 ? '' : names.length === 1 ? (t.one ?? '').replace('{name}', names[0]) : names.length === 2 ? (t.two ?? '').replace('{a}', names[0]).replace('{b}', names[1]) : (t.many ?? '').replace('{n}', names.length);
  if (el.textContent !== text) el.textContent = text;
  clearTimeout(timers.get(el));
  if (who.size) timers.set(el, setTimeout(() => draw(el), Math.max(50, Math.min(...who.values()) - now)));
}

document.addEventListener('zeyn:typing', (e) => {
  const room = e.detail?.room ?? '';
  const send = globalThis.zeyn?.live?.send;
  if (!send) return;
  for (const el of document.querySelectorAll('[data-zeyn-typing]')) {
    if ((el.dataset.room ?? '') !== room) continue;
    const topic = el.dataset.zeynLive ?? '';
    const key = `${topic} ${room}`;
    const now = Date.now();
    if (now - (sent.get(key) ?? 0) < EVERY_MS) continue;
    sent.set(key, now);
    send({ t: 'typing', topic, room });
  }
});

document.addEventListener('zeyn:signal', (e) => {
  const d = e.detail;
  const room = d?.typing?.room;
  const who = d?.typing?.who;
  if (typeof room !== 'string' || typeof who !== 'string') return;
  for (const el of document.querySelectorAll('[data-zeyn-typing]')) {
    if (el.dataset.room !== room || el.dataset.zeynLive !== d.topic || el.dataset.self === who) continue;
    if (!typing.has(el)) typing.set(el, new Map());
    typing.get(el).set(who, Date.now() + SHOWN_MS);
    draw(el);
  }
});
