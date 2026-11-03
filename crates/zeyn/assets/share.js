/**
 * Share buttons (`zeyn::ui::Share`). components.js loads this module the first time a page holds
 * one. It shows the buttons, which the server renders hidden so pages without script have no
 * button that does nothing. Pressing one opens the system share sheet when the browser has the
 * Web Share API, and otherwise copies the link and says so in a toast of the runtime.
 * @module share
 */

/** Shows the share buttons inside `root`. */
function init(root) {
  for (const button of root.querySelectorAll('button[data-zeyn-share][hidden]')) button.hidden = false;
}

/** Shows a toast through zeyn.js, which owns the toast region. */
function toast(message, tone) {
  document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects: [{ effect: 'toast', message, tone }] } }));
}

/** Shares or copies the link of a button. */
async function share(button) {
  const url = new URL(button.dataset.url || location.href, location.href).href;
  const title = button.dataset.title || document.title;
  if (typeof navigator.share === 'function') {
    try {
      await navigator.share({ url, title });
      return;
    } catch (err) {
      if (err?.name === 'AbortError') return;
    }
  }
  try {
    await navigator.clipboard.writeText(url);
    toast(button.dataset.copied, 'success');
  } catch {
    toast(button.dataset.failed, 'danger');
  }
}

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('button[data-zeyn-share]') : null;
  if (button) share(button);
});

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
