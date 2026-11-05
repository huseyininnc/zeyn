/**
 * Reveals a masked value on request (`button[data-zeyn-reveal]`); the server decides and records
 * the read. Loaded only by pages that hold such a button.
 * @module reveal
 */
const B = await import(new URL(`enterprise.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { announce, post, t } = B;

/**
 * Asks the server for the raw value of a masked field. The server checks the unmask permission
 * and audits the request; the value replaces the masked text only in this page.
 */
async function reveal(button) {
  const field = button.dataset.zeynReveal;
  const r = await post('/_zeyn/reveal', { ...address(button), field, key: button.dataset.zeynKey ?? null });
  if (!r.ok || typeof r.data?.value !== 'string') {
    announce(r.data?.message || t('privacy.reveal_failed'), true);
    return;
  }
  const box = button.closest('.z-masked-field');
  const input = box?.querySelector('input[data-zeyn-masked]');
  if (input) {
    input.value = r.data.value;
    input.readOnly = false;
    input.removeAttribute('data-zeyn-masked');
    button.remove();
    input.focus();
  } else {
    const span = button.previousElementSibling;
    if (span?.matches('[data-zeyn-masked]')) {
      span.textContent = r.data.value;
      span.removeAttribute('data-zeyn-masked');
      span.tabIndex = -1;
      button.remove();
      span.focus();
    }
  }
  announce(t('privacy.revealed'));
}

document.addEventListener('click', (e) => {
  const el = e.target instanceof Element ? e.target.closest('button[data-zeyn-reveal]') : null;
  if (!el) return;
  e.preventDefault();
  e.stopPropagation();
  reveal(el);
});

