/**
 * The page socket (ZeynSocket). Linked only in pages whose screen holds `[data-zeyn-live]` (and
 * loaded by `nav.js` when such a screen arrives later); other code may import it to listen for
 * events or call actions.
 *
 * Opens one WebSocket to `/_zeyn/socket` for the whole page; the CSRF token travels as the
 * subprotocol `csrf.TOKEN` next to `zeyn`. It subscribes to the topics of the elements in the
 * page (`{"sub":[..]}`), and when the server notifies one (`{"s":n,"t":topic}`), each element
 * subscribed to it reloads its grids in place (`zeyn:reload`), or the whole screen
 * (`zeyn:refresh`) when it holds no grid, and the nodes that `.listens` to the topic refresh
 * through `topics.js` (`zeyn:topics`). Notifications that arrive within 300 ms are handled once.
 * An element with `data-zeyn-live-self` only subscribes: it refreshes itself on `zeyn:topics`.
 * Typing signals of other users are passed on as `zeyn:signal`.
 *
 * Nodes made with `.on_event` (`[data-zeyn-on]`) refresh or show a toast when their event arrives.
 * `zeyn.socket.on(name, fn)` listens for events (`{"s":n,"e":name,"d":data}`) and returns a
 * function that stops listening; `zeyn.socket.call(action, input)` calls a published action and
 * resolves with its answer or rejects with its problem; `zeyn.socket.state` is `connected`,
 * `reconnecting` or `offline`, announced as `zeyn:socket-state`. `{"resync":true}` (events were
 * lost) refreshes every live element.
 *
 * The connection does not drop for good: the first frame is `{"hello":{"resume":token,"seq":n}}`
 * with the token of the last `welcome` and the last sequence received, so after a drop the server
 * replays what the page missed (or answers `resync`). Received sequences are acknowledged at most
 * once a second (`{"ack":n}`). The server sends `{"p":1}` every 20 s; 45 s without any frame
 * counts as a dead connection. Reconnects wait a random time up to 0.5 s, doubling to 30 s, and
 * happen at once when the browser comes online or the page becomes visible. The socket closes
 * when nothing in the page needs it. When WebSocket fails to open twice in a row (a proxy that
 * strips `Upgrade`), the page switches to Server-Sent Events down and `POST` up with the same
 * frames and resume (`/_zeyn/socket/open`, `/events`, `/send`).
 * @module live
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
/** Close codes after which the socket does not come back (policy, too big). */
const STOP = new Set([1008, 1009]);
/** Without any frame for this long the connection counts as dead. */
const SILENT_MS = 45000;
let socket = null;
let subscribed = new Set();
let wait = 500;
let resume = null;
let lastSeq = 0;
let acked = 0;
let ackTimer = 0;
let heardAt = 0;
/** `ws`, or `sse` after WebSocket failed to open twice in a row. */
let transport = 'ws';
let failures = 0;
let timer = 0;
let state = 'offline';
let nextCall = 1;
const pending = new Set();
let flush = 0;
/** @type {Map<string, Set<(data: any) => void>>} */
const listeners = new Map();
/** @type {Map<number, {resolve: (v: any) => void, reject: (e: any) => void}>} */
const calls = new Map();
/** @type {string[]} Frames waiting for the socket to open. */
const outbox = [];

/** Topics of the elements in the page. */
function wanted() {
  const set = new Set();
  for (const el of document.querySelectorAll('[data-zeyn-live]')) for (const t of el.dataset.zeynLive.split(',')) if (t) set.add(t);
  return set;
}

/** Whether anything in the page needs the socket. */
function needed() {
  return wanted().size > 0 || listeners.size > 0 || calls.size > 0 || !!document.querySelector('[data-zeyn-on]');
}

/**
 * Nodes made with `.on_event` (`[data-zeyn-on="name"]`): a refreshing one listens to a topic named
 * after the event, so the event joins the next `zeyn:topics`; one with `data-zeyn-on-toast` shows
 * its text.
 */
function react(name) {
  for (const el of document.querySelectorAll('[data-zeyn-on]')) {
    if (el.dataset.zeynOn !== name) continue;
    if (el.dataset.zeynOnToast) document.dispatchEvent(new CustomEvent('zeyn:toast', { detail: { message: el.dataset.zeynOnToast } }));
    else {
      pending.add(name);
      flush ||= setTimeout(refresh, 300);
    }
  }
}

function setState(next) {
  if (state === next) return;
  state = next;
  document.dispatchEvent(new CustomEvent('zeyn:socket-state', { detail: { state } }));
}

/** Refreshes the elements of the topics notified since the last run. */
function refresh(all = false) {
  flush = 0;
  for (const el of document.querySelectorAll('[data-zeyn-live]')) {
    if (el.hasAttribute('data-zeyn-live-self')) continue;
    if (!all && !el.dataset.zeynLive.split(',').some((t) => pending.has(t))) continue;
    if (el.querySelector('[data-zeyn-grid]')) el.dispatchEvent(new CustomEvent('zeyn:reload', { bubbles: true }));
    else document.dispatchEvent(new CustomEvent('zeyn:refresh', { cancelable: true }));
  }
  const topics = all ? [...wanted()] : [...pending];
  document.dispatchEvent(new CustomEvent('zeyn:topics', { detail: { topics } }));
  pending.clear();
}

/** Sends a frame now, or when the socket opens. */
function send(frame) {
  const text = JSON.stringify(frame);
  if (socket?.open) socket.send(text);
  else {
    outbox.push(text);
    connect();
  }
}

/** Brings the subscriptions in line with the topics of the page. */
function sync() {
  const next = wanted();
  const add = [...next].filter((t) => !subscribed.has(t));
  const drop = [...subscribed].filter((t) => !next.has(t));
  subscribed = next;
  if (!socket?.open) return;
  if (add.length) socket.send(JSON.stringify({ sub: add }));
  if (drop.length) socket.send(JSON.stringify({ unsub: drop }));
}

/** Acknowledges the last sequence received, at most once a second. */
function ack() {
  ackTimer = 0;
  if (lastSeq > acked && socket?.open) {
    acked = lastSeq;
    socket.send(JSON.stringify({ ack: lastSeq }));
  }
}

function receive(e) {
  heardAt = Date.now();
  let msg;
  try {
    msg = JSON.parse(e.data);
  } catch {
    return;
  }
  if (typeof msg?.s === 'number') {
    lastSeq = Math.max(lastSeq, msg.s);
    ackTimer ||= setTimeout(ack, 1000);
  }
  if (msg?.welcome) {
    resume = msg.welcome.resume;
    if (!msg.welcome.resumed) lastSeq = msg.welcome.seq ?? 0;
    acked = Math.min(acked, lastSeq);
    if (socket && !socket.open) opened(socket);
  } else if (msg?.typing) {
    document.dispatchEvent(new CustomEvent('zeyn:signal', { detail: { topic: msg.t, typing: msg.typing } }));
  } else if (typeof msg?.r === 'number') {
    const call = calls.get(msg.r);
    calls.delete(msg.r);
    if (call) 'err' in msg ? call.reject(msg.err) : call.resolve(msg.ok);
  } else if (typeof msg?.e === 'string') {
    react(msg.e);
    for (const fn of listeners.get(msg.e) ?? []) {
      try {
        fn(msg.d);
      } catch (err) {
        console.error(err);
      }
    }
  } else if (typeof msg?.t === 'string') {
    pending.add(msg.t);
    flush ||= setTimeout(refresh, 300);
  } else if (msg?.resync) {
    refresh(true);
  }
}

/** Marks a connection open: sends what waited and subscribes the page's topics. */
function opened(conn) {
  conn.open = true;
  wait = 500;
  failures = 0;
  heardAt = Date.now();
  setState('connected');
  sync();
  while (outbox.length) conn.send(outbox.shift());
}

/** A connection ended: rejects calls in flight and plans the next attempt. */
function closed(conn, code) {
  if (socket !== conn) return;
  socket = null;
  for (const [, call] of calls) call.reject({ status: 0, type: 'urn:zeyn:problem:offline' });
  calls.clear();
  if (STOP.has(code) || !needed()) {
    setState('offline');
    return;
  }
  setState('reconnecting');
  timer = setTimeout(connect, Math.random() * wait);
  wait = Math.min(wait * 2, 30000);
}

/** The WebSocket transport. Two attempts in a row that never open switch the page to events. */
function connectWs(csrf) {
  const url = new URL(`${BASE}/_zeyn/socket`, location.href);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  const ws = new WebSocket(url, ['zeyn', `csrf.${csrf ?? ''}`]);
  const conn = { open: false, send: (text) => ws.send(text), close: (code, reason) => ws.close(code, reason) };
  socket = conn;
  let reached = false;
  ws.onopen = () => {
    reached = true;
    ws.send(JSON.stringify({ hello: resume ? { resume, seq: lastSeq } : {} }));
    opened(conn);
  };
  ws.onmessage = receive;
  ws.onclose = (e) => {
    if (!reached && ++failures >= 2 && typeof EventSource === 'function') transport = 'sse';
    closed(conn, e.code);
  };
}

/**
 * The fallback transport for networks that block WebSocket: `POST /_zeyn/socket/open` with the
 * CSRF token (and the resume token) gives a single use ticket for the event stream; frames go up
 * one `POST /_zeyn/socket/send` each, in order. The connection counts as open once `welcome`
 * arrives.
 */
async function connectSse(csrf) {
  const conn = { open: false, send: () => {}, close: () => {} };
  socket = conn;
  let res = null;
  try {
    res = await fetch(`${BASE}/_zeyn/socket/open`, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': csrf ?? '' },
      body: JSON.stringify(resume ? { resume, seq: lastSeq } : {}),
    });
  } catch {
    res = null;
  }
  if (socket !== conn) return;
  if (!res?.ok) {
    closed(conn, res && [400, 403, 404].includes(res.status) ? 1008 : 1006);
    return;
  }
  let ticket = '';
  try {
    ticket = (await res.json()).ticket ?? '';
  } catch {
    ticket = '';
  }
  const es = new EventSource(`${BASE}/_zeyn/socket/events?ticket=${encodeURIComponent(ticket)}`);
  let chain = Promise.resolve();
  conn.send = (text) => {
    chain = chain
      .then(() => fetch(`${BASE}/_zeyn/socket/send`, {
        method: 'POST',
        credentials: 'same-origin',
        headers: { 'content-type': 'application/json', 'x-zeyn-csrf': csrf ?? '', 'x-zeyn-socket': resume ?? '' },
        body: text,
      }))
      .catch(() => {});
  };
  conn.close = () => {
    es.close();
    closed(conn, 1000);
  };
  es.onmessage = receive;
  es.onerror = () => {
    es.close();
    closed(conn, 1006);
  };
}

function connect() {
  clearTimeout(timer);
  if (socket) {
    sync();
    return;
  }
  if (!needed() && !outbox.length) return;
  const csrf = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  if (!csrf && window.zeynCsrf) {
    window.zeynCsrf().then((token) => token && connect());
    return;
  }
  subscribed = new Set();
  if (transport === 'sse') connectSse(csrf);
  else connectWs(csrf);
}

/** Reconnects at once while waiting to reconnect. */
function retryNow() {
  if (state === 'reconnecting' && !socket) {
    wait = 500;
    connect();
  }
}

setInterval(() => {
  if (socket?.open && Date.now() - heardAt > SILENT_MS) socket.close(4000, 'silent');
}, 5000);
addEventListener('online', retryNow);
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible') retryNow();
});

/** Closes the socket when nothing in the page needs it any more, else syncs topics. */
function update() {
  if (socket && !needed()) {
    const old = socket;
    socket = null;
    old.close(1000);
    setState('offline');
    return;
  }
  connect();
}

/**
 * Listens for an event the server sends; returns a function that stops listening.
 * @param {string} name
 * @param {(data: any) => void} fn
 */
function on(name, fn) {
  if (!listeners.has(name)) listeners.set(name, new Set());
  listeners.get(name).add(fn);
  connect();
  return () => {
    listeners.get(name)?.delete(fn);
    if (!listeners.get(name)?.size) listeners.delete(name);
  };
}

/**
 * Calls a published action over the socket.
 * @param {string} action
 * @param {any} [input]
 * @returns {Promise<any>} The action's answer; rejects with its problem.
 */
function call(action, input = null) {
  const c = nextCall++;
  const done = new Promise((resolve, reject) => calls.set(c, { resolve, reject }));
  send({ c, a: action, d: input });
  return done;
}

/** Sends a small signal (a typing note) on the open socket; dropped while it is not open. */
function signal(frame) {
  if (socket?.open) socket.send(JSON.stringify(frame));
}

globalThis.zeyn = Object.assign(globalThis.zeyn ?? {}, {
  socket: {
    on,
    call,
    signal,
    get state() {
      return state;
    },
  },
});
document.addEventListener('zeyn:content', update);
connect();
