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
 * `zeyn.socket.on(name, fn)` listens for events (`{"s":n,"e":name,"d":data}`) and returns a
 * function that stops listening; `zeyn.socket.call(action, input)` calls a published action and
 * resolves with its answer or rejects with its problem; `zeyn.socket.state` is `connected`,
 * `reconnecting` or `offline`, announced as `zeyn:socket-state`. `{"resync":true}` (events were
 * lost) refreshes every live element. The socket reconnects with growing pauses (1 s up to 30 s)
 * and closes when nothing in the page needs it.
 * @module live
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
/** Close codes after which the socket does not come back (policy, too big). */
const STOP = new Set([1008, 1009]);
let socket = null;
let subscribed = new Set();
let wait = 1000;
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
  return wanted().size > 0 || listeners.size > 0 || calls.size > 0;
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
  if (socket?.readyState === WebSocket.OPEN) socket.send(text);
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
  if (socket?.readyState !== WebSocket.OPEN) return;
  if (add.length) socket.send(JSON.stringify({ sub: add }));
  if (drop.length) socket.send(JSON.stringify({ unsub: drop }));
}

function receive(e) {
  let msg;
  try {
    msg = JSON.parse(e.data);
  } catch {
    return;
  }
  if (msg?.typing) {
    document.dispatchEvent(new CustomEvent('zeyn:signal', { detail: { topic: msg.t, typing: msg.typing } }));
  } else if (typeof msg?.r === 'number') {
    const call = calls.get(msg.r);
    calls.delete(msg.r);
    if (call) 'err' in msg ? call.reject(msg.err) : call.resolve(msg.ok);
  } else if (typeof msg?.e === 'string') {
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
  const url = new URL(`${BASE}/_zeyn/socket`, location.href);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  const ws = new WebSocket(url, ['zeyn', `csrf.${csrf ?? ''}`]);
  socket = ws;
  subscribed = new Set();
  ws.onopen = () => {
    wait = 1000;
    setState('connected');
    sync();
    while (outbox.length) ws.send(outbox.shift());
  };
  ws.onmessage = receive;
  ws.onclose = (e) => {
    if (socket !== ws) return;
    socket = null;
    for (const [, call] of calls) call.reject({ status: 0, type: 'urn:zeyn:problem:offline' });
    calls.clear();
    if (STOP.has(e.code) || !needed()) {
      setState('offline');
      return;
    }
    setState('reconnecting');
    timer = setTimeout(connect, wait);
    wait = Math.min(wait * 2, 30000);
  };
}

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
  if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(frame));
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
