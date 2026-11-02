/**
 * Change notifications. Linked only in pages whose screen holds `[data-zeyn-live]` (and loaded
 * by `nav.js` when such a screen arrives later).
 *
 * Opens one WebSocket to `/_zeyn/live/topics?t=a,b&csrf=...` for every topic in the page. When
 * the server notifies a topic, each element subscribed to it reloads its grids in place
 * (`zeyn:reload`), or the whole screen (`zeyn:refresh`) when it holds no grid. Notifications that
 * arrive within 300 ms are handled once. The socket reconnects with growing pauses (1 s up to
 * 30 s) and closes when no element subscribes any more.
 * @module live
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
let socket = null;
let topics = '';
let wait = 1000;
let timer = 0;
const pending = new Set();
let flush = 0;

/** Topics of the elements in the page, sorted and comma separated. */
function wanted() {
  const set = new Set();
  for (const el of document.querySelectorAll('[data-zeyn-live]')) for (const t of el.dataset.zeynLive.split(',')) if (t) set.add(t);
  return [...set].sort().join(',');
}

/** Refreshes the elements of the topics notified since the last run. */
function refresh() {
  flush = 0;
  for (const el of document.querySelectorAll('[data-zeyn-live]')) {
    if (!el.dataset.zeynLive.split(',').some((t) => pending.has(t))) continue;
    if (el.querySelector('[data-zeyn-grid]')) el.dispatchEvent(new CustomEvent('zeyn:reload', { bubbles: true }));
    else document.dispatchEvent(new CustomEvent('zeyn:refresh', { cancelable: true }));
  }
  pending.clear();
}

function connect() {
  clearTimeout(timer);
  const next = wanted();
  if (next === topics && socket) return;
  topics = next;
  const old = socket;
  socket = null;
  old?.close(1000);
  if (!topics) return;
  const csrf = document.querySelector('meta[name="zeyn-csrf"]')?.content ?? '';
  const url = new URL(`${BASE}/_zeyn/live/topics?t=${encodeURIComponent(topics)}&csrf=${encodeURIComponent(csrf)}`, location.href);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  const ws = new WebSocket(url);
  socket = ws;
  ws.onopen = () => {
    wait = 1000;
  };
  ws.onmessage = (e) => {
    try {
      pending.add(JSON.parse(e.data).topic);
    } catch {
      return;
    }
    flush ||= setTimeout(refresh, 300);
  };
  ws.onclose = () => {
    if (socket !== ws) return;
    socket = null;
    topics = '';
    timer = setTimeout(connect, wait);
    wait = Math.min(wait * 2, 30000);
  };
}

document.addEventListener('zeyn:content', () => connect());
connect();
