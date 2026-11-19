/**
 * Toggle buttons of the `{{name}}` crate. Loaded only by pages that hold
 * `[data-zeyn-{{component}}-toggle]`, either linked in the page head or loaded by `zeyn.js` when
 * such content arrives. One click listener on the document serves every toggle, so content that
 * arrives later needs no setup. The button is native, so Space and Enter press it.
 * @module {{component}}
 */

document.addEventListener('click', (e) => {
  const button = e.target instanceof Element ? e.target.closest('[data-zeyn-{{component}}-toggle]') : null;
  if (!button) return;
  const on = button.getAttribute('aria-pressed') !== 'true';
  button.setAttribute('aria-pressed', String(on));
  const status = button.parentElement?.querySelector('.z-{{component}}-status');
  if (status) status.textContent = (on ? button.dataset.textOn : button.dataset.textOff) ?? '';
});
