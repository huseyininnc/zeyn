/**
 * Zeyn assistant chat.
 *
 * Drives every `section[data-zeyn-assistant]`: the full page and panel components and the panel
 * of the floating launcher. Answers stream from `POST /_zeyn/assistant/chat` as JSON lines and
 * are shown as they arrive; actions that change data wait for the user's click on a confirmation
 * card. Navigation goes through `zeyn:navigate`, so the page never reloads. Model text is shown
 * as a small Markdown subset built from DOM nodes with `textContent` only: no HTML from the model
 * is ever parsed.
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const MAX_RECORDING_MS = 60_000;

/** State of each chat panel. */
const panels = new WeakMap();

/** Opens the launcher's panel; set when the page has a launcher. */
let openDock = null;

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/** Name of the screen in the main area, so the model knows where the user is. */
function currentScreen() {
  return document.querySelector('.z-main [data-zeyn-screen]')?.dataset.zeynScreen ?? '';
}

function el(tag, className = '', text = null) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

/**
 * Adds inline Markdown (`code`, **bold**, *italic*, _italic_) to `parent` as text nodes and
 * elements. Anything else stays plain text.
 */
function inline(text, parent) {
  const re = /(`[^`\n]+`|\*\*[^*\n]+\*\*|\*[^*\s][^*\n]*\*|_[^_\s][^_\n]*_)/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > last) parent.append(text.slice(last, m.index));
    const t = m[0];
    if (t.startsWith('`')) parent.append(el('code', '', t.slice(1, -1)));
    else if (t.startsWith('**')) parent.append(el('strong', '', t.slice(2, -2)));
    else parent.append(el('em', '', t.slice(1, -1)));
    last = m.index + t.length;
  }
  if (last < text.length) parent.append(text.slice(last));
}

/**
 * The safe Markdown subset as a fragment: paragraphs, line breaks, bullet and numbered lists,
 * headings shown as bold paragraphs, and code blocks. Built from `textContent` only.
 */
function markdown(src) {
  const frag = document.createDocumentFragment();
  const lines = src.replace(/\r/g, '').split('\n');
  let para = null;
  let list = null;
  let code = null;
  const close = () => {
    para = null;
    list = null;
  };
  for (const line of lines) {
    if (code) {
      if (line.trim().startsWith('```')) code = null;
      else code.textContent += (code.textContent ? '\n' : '') + line;
      continue;
    }
    if (line.trim().startsWith('```')) {
      close();
      const pre = el('pre');
      code = el('code');
      pre.append(code);
      frag.append(pre);
      continue;
    }
    const bullet = /^\s*[-*•]\s+(.*)$/.exec(line);
    const numbered = /^\s*\d+[.)]\s+(.*)$/.exec(line);
    if (bullet || numbered) {
      const tag = bullet ? 'UL' : 'OL';
      if (!list || list.tagName !== tag) {
        para = null;
        list = el(tag.toLowerCase());
        frag.append(list);
      }
      const li = el('li');
      inline((bullet ?? numbered)[1], li);
      list.append(li);
      continue;
    }
    if (!line.trim()) {
      close();
      continue;
    }
    const heading = /^#{1,6}\s+(.*)$/.exec(line);
    if (heading) {
      close();
      const p = el('p', 'z-msg-heading');
      const strong = el('strong');
      inline(heading[1], strong);
      p.append(strong);
      frag.append(p);
      continue;
    }
    list = null;
    if (para) para.append(el('br'));
    else {
      para = el('p');
      frag.append(para);
    }
    inline(line, para);
  }
  return frag;
}

/** Reads an error body into one message. */
async function errorText(res, t) {
  try {
    const body = await res.json();
    return body?.message ?? body?.errors?.[0]?.message ?? t.error;
  } catch {
    return t.error;
  }
}

/** Sets up a chat panel once. Panels in the launcher load when first opened. */
function init(root) {
  if (panels.has(root)) return panels.get(root);
  let t = {};
  try {
    t = JSON.parse(root.dataset.zeynTexts ?? '{}');
  } catch {
    t = {};
  }
  const s = {
    root,
    t,
    log: root.querySelector('[data-zeyn-assistant-log]'),
    chips: root.querySelector('[data-zeyn-assistant-chips]'),
    form: root.querySelector('[data-zeyn-assistant-form]'),
    input: root.querySelector('[data-zeyn-assistant-input]'),
    mic: root.querySelector('[data-zeyn-assistant-mic]'),
    stop: root.querySelector('[data-zeyn-assistant-stop]'),
    status: root.querySelector('[data-zeyn-assistant-status]'),
    busy: false,
    loaded: false,
    controller: null,
    lastUser: '',
    stt: 'off',
    tts: 'off',
    recorder: null,
    recognition: null,
  };
  panels.set(root, s);
  s.form.addEventListener('submit', (e) => {
    e.preventDefault();
    const text = s.input.value.trim();
    if (!text || s.busy) return;
    s.input.value = '';
    send(s, text);
  });
  s.input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      s.form.requestSubmit();
    }
  });
  s.stop.addEventListener('click', () => s.controller?.abort());
  s.mic.addEventListener('click', () => toggleMic(s));
  root.querySelector('[data-zeyn-assistant-clear]')?.addEventListener('click', () => clear(s));
  s.chips.addEventListener('click', (e) => {
    const chip = e.target.closest('button[data-prompt]');
    if (chip && !s.busy) send(s, chip.dataset.prompt);
  });
  if (root.dataset.zeynAssistant !== 'dock') load(s);
  return s;
}

/** Loads the history, a pending confirmation, starter prompts and the speech modes. */
async function load(s) {
  if (s.loaded) return;
  s.loaded = true;
  let res;
  try {
    res = await fetch(`${BASE}/_zeyn/assistant/start?screen=${encodeURIComponent(currentScreen())}`, { credentials: 'same-origin' });
  } catch {
    res = null;
  }
  if (!res?.ok) {
    s.log.replaceChildren(el('p', 'z-assistant-empty', s.t.unavailable));
    s.form.querySelectorAll('textarea, button').forEach((b) => (b.disabled = true));
    return;
  }
  const data = await res.json();
  if (data.max) s.input.maxLength = data.max;
  s.stt = data.stt;
  s.tts = data.tts;
  const Recognition = window.SpeechRecognition ?? window.webkitSpeechRecognition;
  const canRecord = !!(navigator.mediaDevices?.getUserMedia && window.MediaRecorder);
  s.mic.hidden = !((s.stt === 'browser' && Recognition) || (s.stt === 'server' && canRecord));
  if (data.messages?.length) {
    s.log.replaceChildren();
    for (const m of data.messages) {
      const b = bubble(s, m.role, m.text);
      if (m.role === 'user') s.lastUser = m.text;
      else finishBubble(s, b, m.text);
    }
  }
  if (data.pending) card(s, data.pending);
  chips(s, data.suggestions ?? []);
  s.log.scrollTop = s.log.scrollHeight;
}

/** Starts a new conversation. */
async function clear(s) {
  if (s.busy) s.controller?.abort();
  await fetch(`${BASE}/_zeyn/assistant/clear`, { method: 'POST', credentials: 'same-origin', headers: { 'x-zeyn-csrf': await csrf() } }).catch(() => null);
  s.log.replaceChildren(el('p', 'z-assistant-empty', s.t.empty));
  s.loaded = false;
  s.chips.replaceChildren();
  await load(s);
  s.input.focus();
}

/** Shows prompt chips. */
function chips(s, list) {
  s.chips.replaceChildren(
    ...list.map((text) => {
      const b = el('button', 'z-chip', text);
      b.type = 'button';
      b.dataset.prompt = text;
      return b;
    }),
  );
}

/** Adds a message bubble to the log. */
function bubble(s, role, text) {
  s.log.querySelector('.z-assistant-empty')?.remove();
  const b = el('div', `z-msg z-msg-${role}`);
  b.append(el('span', 'z-sr-only', `${role === 'user' ? s.t.you : s.t.assistant}: `));
  const body = el('div', 'z-msg-body');
  if (role === 'user') body.textContent = text;
  b.append(body);
  s.log.append(b);
  s.log.scrollTop = s.log.scrollHeight;
  return b;
}

/** Final text of an assistant bubble and its actions: read aloud, copy, retry. */
function finishBubble(s, b, text) {
  b.removeAttribute('aria-busy');
  b.querySelector('.z-msg-typing')?.remove();
  const body = b.querySelector('.z-msg-body');
  body.replaceChildren(markdown(text));
  if (!text.trim()) return;
  const actions = el('div', 'z-msg-actions');
  const action = (label, handler) => {
    const button = el('button', 'z-msg-action', label);
    button.type = 'button';
    button.addEventListener('click', () => handler(button));
    actions.append(button);
    return button;
  };
  if (s.tts !== 'off' && (s.tts === 'server' || 'speechSynthesis' in window)) action(s.t.read, (button) => speak(s, text, button));
  action(s.t.copy, async (button) => {
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = s.t.copied;
      setTimeout(() => (button.textContent = s.t.copy), 1500);
    } catch {
      return;
    }
  });
  if (s.lastUser) action(s.t.retry, () => !s.busy && send(s, s.lastUser));
  b.append(actions);
}

function setBusy(s, on) {
  s.busy = on;
  s.stop.hidden = !on;
  s.root.querySelector('.z-assistant-send').hidden = on;
  s.root.querySelectorAll('.z-chip').forEach((c) => (c.disabled = on));
}

/** Sends the user's message and streams the answer. */
async function send(s, text) {
  s.lastUser = text;
  s.chips.replaceChildren();
  bubble(s, 'user', text);
  await stream(s, '/_zeyn/assistant/chat', { message: text, screen: currentScreen() });
}

/**
 * Posts and reads the answer as JSON lines: text pieces, tool activity, navigation,
 * suggestions, confirmation cards, errors.
 */
async function stream(s, url, body) {
  setBusy(s, true);
  const b = bubble(s, 'assistant', '');
  b.setAttribute('aria-busy', 'true');
  const typing = el('span', 'z-msg-typing');
  typing.append(el('span', 'z-sr-only', s.t.thinking));
  b.querySelector('.z-msg-body').append(typing);
  s.controller = new AbortController();
  let text = '';
  let frame = 0;
  const later = [];
  const docked = s.root.dataset.zeynAssistant === 'dock';
  const paint = () => {
    frame = 0;
    const body = b.querySelector('.z-msg-body');
    body.replaceChildren(markdown(text));
    s.log.scrollTop = s.log.scrollHeight;
  };
  const handle = (e) => {
    switch (e.t) {
      case 'text':
        text += e.v;
        if (!frame) frame = requestAnimationFrame(paint);
        break;
      case 'tool': {
        const note = el('p', 'z-msg-tool', s.t.working);
        b.before(note);
        break;
      }
      case 'navigate':
        if (docked) go(e.url, e.filters ?? [], false);
        else later.push(e);
        break;
      case 'suggest':
        chips(s, e.v ?? []);
        break;
      case 'confirm':
        card(s, e);
        break;
      case 'resolved':
        s.log.querySelectorAll(`.z-assistant-confirm[data-id="${CSS.escape(e.id)}"] button`).forEach((x) => (x.disabled = true));
        break;
      case 'error':
        b.append(el('p', 'z-msg-error', e.message ?? s.t.error));
        s.status.textContent = e.message ?? s.t.error;
        break;
      default:
    }
  };
  try {
    const res = await fetch(BASE + url, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify(body),
      signal: s.controller.signal,
    });
    if (!res.ok) {
      handle({ t: 'error', message: await errorText(res, s.t) });
    } else {
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buf = '';
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        buf += decoder.decode(value, { stream: true });
        let nl;
        while ((nl = buf.indexOf('\n')) >= 0) {
          const line = buf.slice(0, nl).trim();
          buf = buf.slice(nl + 1);
          if (!line) continue;
          try {
            handle(JSON.parse(line));
          } catch {
            continue;
          }
        }
      }
    }
  } catch (err) {
    if (err?.name === 'AbortError') {
      b.append(el('p', 'z-msg-note', s.t.stopped));
      s.status.textContent = s.t.stopped;
    } else handle({ t: 'error', message: s.t.error });
  } finally {
    if (frame) cancelAnimationFrame(frame);
    finishBubble(s, b, text);
    if (!text && !b.querySelector('.z-msg-error, .z-msg-note')) b.remove();
    s.controller = null;
    setBusy(s, false);
    s.log.scrollTop = s.log.scrollHeight;
    for (const e of later) go(e.url, e.filters ?? [], true);
  }
}

/**
 * Opens a screen without a page load, then applies the model's filters to its grid. A chat that
 * lives in the screen itself goes away with it, so the conversation moves to the launcher's panel.
 */
function go(url, filters, reopen) {
  document.addEventListener(
    'zeyn:content',
    () =>
      setTimeout(() => {
        if (filters.length) document.dispatchEvent(new CustomEvent('zeyn:filter', { detail: { filters } }));
        if (reopen && !document.querySelector('.z-main [data-zeyn-assistant]')) openDock?.(true, false);
      }, 0),
    { once: true },
  );
  const handled = !document.dispatchEvent(new CustomEvent('zeyn:navigate', { detail: { url }, cancelable: true }));
  if (!handled) location.assign(url);
}

/** The confirmation card of an action that waits for the user. */
function card(s, e) {
  s.log.querySelector('.z-assistant-empty')?.remove();
  const box = el('div', 'z-assistant-confirm');
  box.dataset.id = e.id;
  box.setAttribute('role', 'group');
  const titleId = `z-confirm-${Math.random().toString(36).slice(2)}`;
  box.setAttribute('aria-labelledby', titleId);
  const h = el('p', 'z-assistant-confirm-title', s.t.confirm_title);
  h.id = titleId;
  box.append(h, el('p', 'z-assistant-confirm-what', e.title ?? ''));
  if (e.details?.length) {
    const dl = el('dl');
    for (const d of e.details) dl.append(el('dt', '', d.name), el('dd', '', d.value));
    box.append(dl);
  }
  box.append(el('p', 'z-assistant-confirm-question', e.question ?? ''));
  const actions = el('div', 'z-assistant-confirm-actions');
  const no = el('button', 'z-button', s.t.decline);
  const yes = el('button', 'z-button z-primary', s.t.confirm);
  no.type = yes.type = 'button';
  const answer = async (approve) => {
    if (s.busy) return;
    yes.disabled = no.disabled = true;
    box.classList.add(approve ? 'z-confirmed' : 'z-declined');
    if (!approve) box.append(el('p', 'z-msg-note', s.t.declined));
    await stream(s, '/_zeyn/assistant/confirm', { id: e.id, approve });
  };
  yes.addEventListener('click', () => answer(true));
  no.addEventListener('click', () => answer(false));
  actions.append(no, yes);
  box.append(actions);
  s.log.append(box);
  s.log.scrollTop = s.log.scrollHeight;
  s.status.textContent = `${s.t.confirm_title}: ${e.title ?? ''}`;
  no.focus({ preventScroll: true });
}

/** Reads an answer aloud with the browser's voice or the server's. */
async function speak(s, text, button) {
  const plain = text.replace(/[`*_#>]/g, '');
  if (s.tts === 'browser') {
    if (speechSynthesis.speaking) {
      speechSynthesis.cancel();
      return;
    }
    const u = new SpeechSynthesisUtterance(plain);
    u.lang = document.documentElement.lang || 'en';
    button.setAttribute('aria-pressed', 'true');
    u.onend = u.onerror = () => button.setAttribute('aria-pressed', 'false');
    speechSynthesis.speak(u);
    return;
  }
  try {
    const res = await fetch(`${BASE}/_zeyn/assistant/tts`, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify({ text: plain }),
    });
    if (!res.ok) throw new Error(await errorText(res, s.t));
    const audio = new AudioContext();
    const buffer = await audio.decodeAudioData(await res.arrayBuffer());
    const source = audio.createBufferSource();
    source.buffer = buffer;
    source.connect(audio.destination);
    button.setAttribute('aria-pressed', 'true');
    source.onended = () => {
      button.setAttribute('aria-pressed', 'false');
      audio.close();
    };
    source.start();
  } catch (err) {
    s.status.textContent = err?.message || s.t.error;
  }
}

/** Starts or stops speech input: the browser's recognition, or a recording sent to the server. */
async function toggleMic(s) {
  const on = s.mic.getAttribute('aria-pressed') === 'true';
  const set = (pressed) => {
    s.mic.setAttribute('aria-pressed', String(pressed));
    const label = pressed ? s.t.mic_stop : s.t.mic;
    s.mic.setAttribute('aria-label', label);
    s.mic.title = label;
    s.status.textContent = pressed ? s.t.listening : '';
  };
  if (on) {
    s.recognition?.stop();
    if (s.recorder?.state === 'recording') s.recorder.stop();
    return;
  }
  if (s.stt === 'browser') {
    const Recognition = window.SpeechRecognition ?? window.webkitSpeechRecognition;
    if (!Recognition) return;
    const rec = new Recognition();
    rec.lang = document.documentElement.lang || 'en';
    rec.interimResults = true;
    rec.onresult = (e) => {
      s.input.value = [...e.results].map((r) => r[0].transcript).join(' ');
    };
    rec.onend = rec.onerror = () => {
      set(false);
      s.recognition = null;
      s.input.focus();
    };
    s.recognition = rec;
    set(true);
    rec.start();
    return;
  }
  let media;
  try {
    media = await navigator.mediaDevices.getUserMedia({ audio: true });
  } catch {
    s.status.textContent = s.t.error;
    return;
  }
  const parts = [];
  const recorder = new MediaRecorder(media);
  s.recorder = recorder;
  const limit = setTimeout(() => recorder.state === 'recording' && recorder.stop(), MAX_RECORDING_MS);
  recorder.ondataavailable = (e) => e.data.size && parts.push(e.data);
  recorder.onstop = async () => {
    clearTimeout(limit);
    media.getTracks().forEach((track) => track.stop());
    set(false);
    const blob = new Blob(parts, { type: recorder.mimeType || 'audio/webm' });
    try {
      const res = await fetch(`${BASE}/_zeyn/assistant/stt`, {
        method: 'POST',
        credentials: 'same-origin',
        headers: { 'content-type': blob.type.split(';')[0], 'x-zeyn-csrf': await csrf() },
        body: blob,
      });
      if (!res.ok) throw new Error(await errorText(res, s.t));
      s.input.value = (await res.json()).text ?? '';
      s.input.focus();
    } catch (err) {
      s.status.textContent = err?.message || s.t.error;
    }
  };
  set(true);
  recorder.start();
}

/** The floating launcher: opens and closes the side panel, keeps focus where it belongs. */
function launcher() {
  const button = document.querySelector('[data-zeyn-assistant-launch]');
  const dock = document.getElementById('z-assistant-dock');
  if (!button || !dock) return;
  const panel = dock.querySelector('[data-zeyn-assistant]');
  const open = (show, focus = true) => {
    dock.hidden = !show;
    button.setAttribute('aria-expanded', String(show));
    if (show) {
      const s = init(panel);
      s.loaded = false;
      if (!s.busy) {
        s.log.replaceChildren(el('p', 'z-assistant-empty', s.t.empty));
        load(s);
      }
      if (focus) s.input.focus();
    } else if (focus) button.focus();
  };
  openDock = open;
  button.addEventListener('click', () => open(dock.hidden));
  dock.querySelector('[data-zeyn-assistant-close]')?.addEventListener('click', () => open(false));
  dock.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault();
      open(false);
    }
  });
  const sync = () => {
    const page = document.querySelector('.z-main [data-zeyn-assistant]');
    button.hidden = !!page;
    if (page && !dock.hidden) open(false, false);
  };
  document.addEventListener('zeyn:content', sync);
  sync();
}

/** Sets up chat panels in new content (navigation, lazy panels). */
function scan(root) {
  root.querySelectorAll?.('section[data-zeyn-assistant]:not([data-zeyn-assistant="dock"])').forEach(init);
}

document.addEventListener('zeyn:content', (e) => scan(e.detail ?? document));
scan(document);
launcher();
