/**
 * Toggle buttons of the `zeyn-example-widget` crate. Loaded only by pages that hold
 * `[data-zeyn-widget-toggle]`, either linked in the page head or loaded by `zeyn.js` when such
 * content arrives. One click listener on the document serves every toggle, so content that
 * arrives later needs no setup. The button is native, so Space and Enter press it.
 * @module widget
 */

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('[data-zeyn-widget-toggle]') : null;
  if (!button) return;
  const on = button.getAttribute('aria-pressed') !== 'true';
  button.setAttribute('aria-pressed', String(on));
  const status = button.parentElement?.querySelector('.z-widget-status');
  if (status) status.textContent = (on ? button.dataset.textOn : button.dataset.textOff) ?? '';
});
