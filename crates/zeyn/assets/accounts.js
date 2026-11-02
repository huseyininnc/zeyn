/**
 * Passkeys for the Zeyn accounts system: buttons marked `data-zeyn-passkey="login"` sign in with a
 * passkey and `data-zeyn-passkey="add"` add one to the signed in account. The server makes the
 * challenge and checks the answer; this script only carries them between the server and
 * `navigator.credentials`. Moves go through `nav.js`, never a page load.
 */
const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const API = `${BASE}/_zeyn/account`;

/** The CSRF token of the page. */
function csrf() {
  return document.querySelector('meta[name="zeyn-csrf"]')?.content ?? '';
}

/** Base64url text to bytes. */
function bytes(text) {
  const b64 = text.replace(/-/g, '+').replace(/_/g, '/');
  const bin = atob(b64 + '='.repeat((4 - (b64.length % 4)) % 4));
  return Uint8Array.from(bin, (c) => c.charCodeAt(0));
}

/** Bytes to base64url text. */
function text(buffer) {
  let bin = '';
  for (const b of new Uint8Array(buffer)) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** Posts JSON and returns the answer as JSON, or throws with the server's message. */
async function post(path, body) {
  const res = await fetch(`${API}${path}`, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'content-type': 'application/json', 'x-zeyn-csrf': csrf() },
    body: JSON.stringify(body ?? {}),
  });
  const data = await res.json().catch(() => null);
  if (!res.ok) throw new Error(data?.detail ?? '');
  return data;
}

/** Request options with the binary fields decoded. */
function requestOptions(o) {
  return {
    ...o,
    challenge: bytes(o.challenge),
    allowCredentials: (o.allowCredentials ?? []).map((c) => ({ ...c, id: bytes(c.id) })),
  };
}

/** Creation options with the binary fields decoded. */
function creationOptions(o) {
  return {
    ...o,
    challenge: bytes(o.challenge),
    user: { ...o.user, id: bytes(o.user.id) },
    excludeCredentials: (o.excludeCredentials ?? []).map((c) => ({ ...c, id: bytes(c.id) })),
  };
}

/** A credential as WebAuthn level 3 JSON, also in browsers without `toJSON`. */
function credentialJson(c) {
  if (typeof c.toJSON === 'function') return c.toJSON();
  const r = c.response;
  const response = { clientDataJSON: text(r.clientDataJSON) };
  if (r.attestationObject) {
    response.attestationObject = text(r.attestationObject);
    response.transports = typeof r.getTransports === 'function' ? r.getTransports() : [];
  }
  if (r.authenticatorData) {
    response.authenticatorData = text(r.authenticatorData);
    response.signature = text(r.signature);
    if (r.userHandle) response.userHandle = text(r.userHandle);
  }
  return { id: c.id, rawId: text(c.rawId), type: c.type, response };
}

/** Shows a problem next to the button, in a live region. */
function fail(button, message) {
  const out = button.closest('section, .z-screen, form, div')?.querySelector('[data-zeyn-passkey-error]') ?? document.querySelector('[data-zeyn-passkey-error]');
  if (out) out.textContent = message || button.dataset.failed || '';
}

/** Goes to `url`, loading the whole app frame again, as after any sign in. */
function goFrame(url) {
  history.pushState({ zeyn: true }, '', url);
  if (document.dispatchEvent(new CustomEvent('zeyn:refresh-page', { cancelable: true }))) location.assign(url);
}

async function signIn(button) {
  const options = await post('/passkeys/requestOptions');
  const credential = await navigator.credentials.get({ publicKey: requestOptions(options) });
  await post('/passkeys/login?useSessionCookies=true', { credentialJson: JSON.stringify(credentialJson(credential)) });
  goFrame(button.dataset.return || `${BASE}/`);
}

async function add(button) {
  const name = document.querySelector('[data-zeyn-passkey-name]')?.value?.trim() || null;
  const options = await post('/manage/passkeys/creationOptions');
  const credential = await navigator.credentials.create({ publicKey: creationOptions(options) });
  await post('/manage/passkeys', { credentialJson: JSON.stringify(credentialJson(credential)), name });
  if (document.dispatchEvent(new CustomEvent('zeyn:refresh', { cancelable: true }))) location.reload();
}

document.addEventListener('click', async (e) => {
  const button = e.target.closest?.('[data-zeyn-passkey]');
  if (!button) return;
  e.preventDefault();
  if (!window.PublicKeyCredential || !navigator.credentials) {
    fail(button, button.dataset.unsupported);
    return;
  }
  button.disabled = true;
  try {
    if (button.dataset.zeynPasskey === 'login') await signIn(button);
    else await add(button);
  } catch (err) {
    fail(button, err?.name === 'NotAllowedError' ? button.dataset.failed : err?.message);
  } finally {
    button.disabled = false;
  }
});
