/**
 * Share and copy buttons (`zeyn::ui::Share`, the copy button of `zeyn::ui::CodeView`). Loaded
 * only by pages that hold `[data-zeyn-share]` or `[data-zeyn-copy]`. It shows the buttons, which
 * the server renders hidden so pages without script have no button that does nothing. A share
 * button opens the system share sheet when the browser has the Web Share API, and otherwise
 * copies the link and says so in a toast of the runtime. A copy button copies the text of the
 * element its `data-zeyn-copy` names: the lines (`.z-codeview-l`) it holds, one per line, or its
 * whole text.
 * @module share
 */

/** Shows the share and copy buttons inside `root`. */
function init(root) {
  for (const button of root.querySelectorAll('button[data-zeyn-share][hidden], button[data-zeyn-copy][hidden]')) button.hidden = false;
}

/** Shows a toast through zeyn.js, which owns the toast region. */
function toast(message, tone) {
  document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects: [{ effect: 'toast', message, tone }] } }));
}

/** Writes text to the clipboard and says whether it worked. */
async function write(text, button) {
  try {
    await navigator.clipboard.writeText(text);
    toast(button.dataset.copied, 'success');
  } catch {
    toast(button.dataset.failed, 'danger');
  }
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
  await write(url, button);
}

/** Copies the text a copy button names. */
function copy(button) {
  const target = document.getElementById(button.dataset.zeynCopy);
  if (!target) return;
  const lines = target.querySelectorAll('.z-codeview-l');
  write(lines.length ? [...lines].map((l) => l.textContent).join('\n') : target.textContent, button);
}

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('button[data-zeyn-share], button[data-zeyn-copy]') : null;
  if (button?.hasAttribute('data-zeyn-copy')) copy(button);
  else if (button) share(button);
});

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
