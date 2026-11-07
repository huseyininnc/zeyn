/**
 * Chat composers (`zeyn::ui::Composer`). Loaded only by pages that hold `[data-zeyn-composer]`.
 *
 * The text area grows with its text up to the height its stylesheet allows. The send key comes
 * from `data-send-key`: `enter` sends with Enter (Shift+Enter is a new line), `ctrl-enter` with
 * Ctrl or Cmd+Enter. The send button stays disabled while the message is empty or too long; a
 * counter shows near the limit. Sending posts `{ text, attachments, mentions }` to
 * `POST /_zeyn/data` (key `send`); after a success the composer fires `zeyn:chat-sent` with the
 * conversation and the text and names its topics (`zeyn:topics`), so threads of the conversation
 * fetch the message. Typing fires `zeyn:typing` at most every 3 seconds; only `typing.js`, which
 * loads with a realtime typing indicator, sends it anywhere.
 *
 * Mentions: `@` and up to 50 characters search people through the key `mentions`, 200 ms after
 * the last key; a newer search aborts the older one. The list is an APG listbox driven from the
 * text area (`aria-activedescendant`): arrows move, Enter or Tab picks, Escape closes.
 *
 * Drafts: with `data-zeyn-remember` the unsent text of each conversation is kept in the client
 * preference store (`state.js`) under the key's name and a hash of the conversation, at most 20
 * conversations, the oldest dropped first, and removed once sent.
 * @module composer
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const VERSION = new URL(import.meta.url).search;
const TYPING_MS = 3000;
const SEARCH_MS = 200;
const DRAFT_MS = 400;
const MAX_DRAFTS = 20;
const MAX_QUERY = 50;
/** @type {WeakMap<Element, object>} State per composer. */
const composers = new WeakMap();

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

/** Calls data source `key` of the composer: `{ ok, body }`. */
async function data(form, key, input, signal) {
  const screen = form.closest('[data-zeyn-screen]');
  const body = {
    screen: screen?.dataset.zeynScreen ?? null,
    params: parse(screen?.dataset.zeynParams ?? 'null'),
    node: form.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
    key,
    input,
  };
  try {
    const res = await fetch(BASE + '/_zeyn/data', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify(body),
      signal,
    });
    return { ok: res.ok, body: parse(await res.text()) };
  } catch (err) {
    return { ok: false, body: null, aborted: err?.name === 'AbortError' };
  }
}

/** A short hash of a conversation id, so drafts are kept without the id itself. */
function hash(text) {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, '0');
}

/** Says `text` in the composer's live region. */
function say(s, text) {
  s.status.textContent = '';
  requestAnimationFrame(() => (s.status.textContent = text));
}

/** Files the upload field accepted, from its JSON value. */
function files(s) {
  const value = s.form.querySelector('.z-upload-value')?.value ?? '';
  const list = parse(value || '[]');
  return Array.isArray(list) ? list : [];
}

/** Grows the text area with its text, and updates the counter and the send button. */
function sync(s) {
  const ta = s.text;
  ta.style.height = 'auto';
  ta.style.height = `${ta.scrollHeight}px`;
  const n = [...ta.value.trim()].length;
  const left = s.max - [...ta.value].length;
  const over = left < 0;
  s.count.textContent = over ? s.t.over.replace('{n}', -left) : left <= Math.max(20, s.max / 10) ? s.t.left.replace('{n}', left) : '';
  s.count.classList.toggle('z-composer-over', over);
  s.button.setAttribute('aria-disabled', String(over || (n === 0 && files(s).length === 0) || s.busy));
  ta.setAttribute('aria-invalid', String(over));
}

/** The draft keys of a composer: the index of conversations and this conversation's text. */
function draftKeys(s) {
  const d = s.form.dataset;
  const ms = Number(d.zeynRememberFor);
  const name = d.zeynRemember;
  if (!name || !(ms > 0)) return null;
  const purpose = d.zeynRememberPurpose === 'preference' ? 'preference' : 'necessary';
  const max = Number(d.zeynRememberMax) || 200;
  return { index: { name, ms, purpose }, text: { name: `${name}-${hash(s.room)}`, ms, purpose, max } };
}

/** The preference store, imported the first time a draft is read or written. */
let store = null;
function state() {
  store ??= import(`${BASE}/_zeyn/state.js${VERSION}`);
  return store;
}

/** Saves or removes the draft of the conversation, keeping at most `MAX_DRAFTS` of them. */
async function saveDraft(s) {
  const keys = draftKeys(s);
  if (!keys) return;
  const st = await state();
  const text = s.text.value;
  const index = String(st.get(keys.index) ?? '').split(',').filter((h) => /^[0-9a-f]{8}$/.test(h));
  const me = hash(s.room);
  const others = index.filter((h) => h !== me);
  if (!text.trim()) {
    st.remove(keys.text);
    if (others.length) await st.set(keys.index, others.join(','));
    else st.remove(keys.index);
    return;
  }
  const kept = [...others, me];
  while (kept.length > MAX_DRAFTS) {
    const gone = kept.shift();
    st.remove({ ...keys.text, name: `${keys.index.name}-${gone}` });
  }
  await st.set(keys.text, text.slice(0, keys.text.max));
  await st.set(keys.index, kept.join(','));
}

/** Puts the kept draft back into an empty text area. */
async function restoreDraft(s) {
  const keys = draftKeys(s);
  if (!keys) return;
  const st = await state();
  const text = st.get(keys.text);
  if (typeof text === 'string' && !s.text.value) {
    s.text.value = text;
    sync(s);
    say(s, s.t.restored);
  }
}

/** Closes the mention list. */
function closePeople(s) {
  if (!s.people) return;
  s.people.hidden = true;
  s.people.replaceChildren();
  s.text.removeAttribute('aria-activedescendant');
  s.found = [];
  s.active = -1;
}

/** Marks option `i` of the mention list as the active one. */
function activate(s, i) {
  const options = [...s.people.children];
  if (!options.length) return;
  s.active = (i + options.length) % options.length;
  options.forEach((o, k) => o.setAttribute('aria-selected', String(k === s.active)));
  s.text.setAttribute('aria-activedescendant', options[s.active].id);
  options[s.active].scrollIntoView({ block: 'nearest' });
}

/** The `@text` right before the caret, or `null`. */
function mentionAt(s) {
  const before = s.text.value.slice(0, s.text.selectionStart);
  const m = /(^|\s)@([^\s@]{0,50})$/.exec(before);
  return m ? { text: m[2], start: before.length - m[2].length - 1 } : null;
}

/** Searches people for the `@text` before the caret; a newer search aborts this one. */
async function search(s) {
  const at = mentionAt(s);
  s.searchCtl?.abort();
  if (!at || [...at.text].length > MAX_QUERY) {
    closePeople(s);
    return;
  }
  const ctl = new AbortController();
  s.searchCtl = ctl;
  const r = await data(s.form, 'mentions', { text: at.text }, ctl.signal);
  if (ctl.signal.aborted || r.aborted) return;
  const items = r.ok && Array.isArray(r.body?.items) ? r.body.items : [];
  s.found = items;
  s.people.replaceChildren();
  items.forEach((item, i) => {
    const li = document.createElement('li');
    li.id = `${s.people.id}-${i}`;
    li.setAttribute('role', 'option');
    li.setAttribute('aria-selected', 'false');
    li.className = 'z-composer-person';
    li.textContent = String(item.label ?? '');
    if (item.hint) {
      const hint = document.createElement('span');
      hint.className = 'z-composer-person-hint';
      hint.textContent = ` ${item.hint}`;
      li.append(hint);
    }
    li.addEventListener('mousedown', (e) => {
      e.preventDefault();
      pick(s, i);
    });
    s.people.append(li);
  });
  s.people.hidden = items.length === 0;
  say(s, items.length ? s.t.people.replace('{n}', items.length) : s.t.none);
  if (items.length) activate(s, 0);
  else s.text.removeAttribute('aria-activedescendant');
}

/** Puts person `i` of the list into the text in place of the `@text`. */
function pick(s, i) {
  const item = s.found[i];
  const at = mentionAt(s);
  if (!item || !at) return closePeople(s);
  const label = String(item.label ?? '');
  const ta = s.text;
  const insert = `@${label} `;
  ta.value = ta.value.slice(0, at.start) + insert + ta.value.slice(ta.selectionStart);
  const caret = at.start + insert.length;
  ta.setSelectionRange(caret, caret);
  if (!s.mentions.some((m) => m.id === item.id)) s.mentions.push({ id: item.id, label });
  closePeople(s);
  sync(s);
  scheduleDraft(s);
}

/** Saves the draft a little after the last key. */
function scheduleDraft(s) {
  clearTimeout(s.draftTimer);
  s.draftTimer = setTimeout(() => saveDraft(s), DRAFT_MS);
}

/** Sends the message. */
async function send(s) {
  sync(s);
  if (s.busy || s.button.getAttribute('aria-disabled') === 'true') return;
  const text = s.text.value.trim();
  const attachments = files(s);
  const mentions = s.mentions.filter((m) => text.includes(`@${m.label}`)).map((m) => m.id);
  s.busy = true;
  s.form.setAttribute('aria-busy', 'true');
  sync(s);
  const r = await data(s.form, 'send', { text, attachments, mentions });
  s.busy = false;
  s.form.removeAttribute('aria-busy');
  if (!r.ok) {
    sync(s);
    say(s, r.body?.message || s.t.failed);
    return;
  }
  s.text.value = '';
  s.mentions = [];
  const value = s.form.querySelector('.z-upload-value');
  if (value) {
    value.value = '[]';
    s.form.querySelectorAll('.z-upload-item').forEach((li) => li.remove());
  }
  clearTimeout(s.draftTimer);
  saveDraft(s);
  sync(s);
  say(s, s.t.sent);
  s.text.focus();
  document.dispatchEvent(new CustomEvent('zeyn:chat-sent', { detail: { room: s.room, text } }));
  document.dispatchEvent(new CustomEvent('zeyn:topics', { detail: { topics: s.topics } }));
}

/** Sets up the composers inside `root`. */
function init(root) {
  root.querySelectorAll?.('[data-zeyn-composer]:not([data-ready])').forEach((form) => {
    form.dataset.ready = '';
    const s = {
      form,
      text: form.querySelector('.z-composer-text'),
      button: form.querySelector('.z-composer-send'),
      count: form.querySelector('.z-composer-count'),
      status: form.querySelector('[data-zeyn-composer-status]'),
      people: form.querySelector('.z-composer-people'),
      room: form.dataset.room ?? '',
      topics: (form.dataset.topics ?? '').split(',').filter(Boolean),
      key: form.dataset.sendKey === 'ctrl-enter' ? 'ctrl-enter' : 'enter',
      max: Number(form.dataset.max) || 4000,
      t: parse(form.dataset.texts ?? '{}') ?? {},
      busy: false,
      mentions: [],
      found: [],
      active: -1,
      typed: 0,
      searchTimer: 0,
      draftTimer: 0,
      searchCtl: null,
    };
    if (!s.text || !s.button) return;
    composers.set(form, s);
    form.addEventListener('submit', (e) => {
      e.preventDefault();
      send(s);
    });
    s.text.addEventListener('input', () => {
      sync(s);
      scheduleDraft(s);
      const now = Date.now();
      if (s.text.value.trim() && now - s.typed > TYPING_MS) {
        s.typed = now;
        document.dispatchEvent(new CustomEvent('zeyn:typing', { detail: { room: s.room } }));
      }
      if (s.people) {
        clearTimeout(s.searchTimer);
        if (mentionAt(s)) s.searchTimer = setTimeout(() => search(s), SEARCH_MS);
        else {
          s.searchCtl?.abort();
          closePeople(s);
        }
      }
    });
    s.text.addEventListener('keydown', (e) => {
      if (e.isComposing) return;
      const open = s.people && !s.people.hidden && s.found.length;
      if (open) {
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          e.preventDefault();
          activate(s, s.active + (e.key === 'ArrowDown' ? 1 : -1));
          return;
        }
        if (e.key === 'Enter' || e.key === 'Tab') {
          e.preventDefault();
          pick(s, Math.max(0, s.active));
          return;
        }
        if (e.key === 'Escape') {
          e.preventDefault();
          closePeople(s);
          return;
        }
      }
      if (e.key !== 'Enter') return;
      const mod = e.ctrlKey || e.metaKey;
      const sends = s.key === 'enter' ? !e.shiftKey && !mod && !e.altKey : mod;
      if (sends) {
        e.preventDefault();
        send(s);
      }
    });
    s.text.addEventListener('blur', () => setTimeout(() => closePeople(s), 150));
    form.addEventListener('change', () => sync(s));
    form.addEventListener('zeyn:files', () => sync(s));
    sync(s);
    restoreDraft(s);
  });
}

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
