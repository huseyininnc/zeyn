/**
 * Development only (`zeyn dev` sets `ZEYN_DEV=1`): the server announces its build over
 * `/_zeyn/dev/events`. When `zeyn dev` restarts it with a new build, the app frame is refreshed in
 * place through nav.js, without a page load. Never served in production.
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
let build = document.querySelector('meta[name="zeyn-build"]')?.content ?? '';
const source = new EventSource(`${BASE}/_zeyn/dev/events`);

source.addEventListener('build', (e) => {
  if (!build) build = e.data;
  if (e.data === build) return;
  build = e.data;
  if (document.dispatchEvent(new CustomEvent('zeyn:refresh-page', { cancelable: true }))) location.reload();
});
