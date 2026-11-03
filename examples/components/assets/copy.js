/**
 * Copy buttons of the component catalog. Each example has a hidden button next to its code; this
 * script shows the buttons and copies the code of the one pressed. Without the script, or without
 * the clipboard API, the buttons stay hidden and the code can still be selected by hand.
 * @module copy
 */

const supported = Boolean(navigator.clipboard?.writeText);

/** Shows the copy buttons inside `root`. */
function reveal(root) {
  if (!supported) return;
  for (const button of root.querySelectorAll?.('[data-cx-copy][hidden]') ?? []) button.hidden = false;
}

document.addEventListener('click', async (e) => {
  const button = e.target.closest?.('[data-cx-copy]');
  if (!button) return;
  const wrap = button.closest('.cx-code-wrap');
  const code = wrap?.querySelector('code')?.textContent ?? '';
  const status = wrap?.querySelector('[data-cx-copy-status]');
  try {
    await navigator.clipboard.writeText(code);
    button.textContent = button.dataset.textCopied;
    if (status) status.textContent = button.dataset.textCopied;
    setTimeout(() => {
      button.textContent = button.dataset.textCopy;
      if (status) status.textContent = '';
    }, 1500);
  } catch {
    button.hidden = true;
  }
});

document.addEventListener('zeyn:content', (e) => reveal(e.detail instanceof Element ? e.detail : document));
reveal(document);
