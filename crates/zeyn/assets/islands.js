/**
 * Island loader. Linked only in pages that hold an island (`div[data-zeyn-island]`).
 *
 * Each island loads when its placeholder comes near the viewport (`data-zeyn-load="visible"`),
 * on the first pointer, touch or focus (`interaction`) or when the browser is idle (`idle`).
 * Loading never blocks the page: the module and its WebAssembly are fetched once per island name,
 * then `mount(root)` runs. The fallback content stays until the island mounts, and for good when
 * it cannot load. When a mounted placeholder leaves the document, `unmount(root)` runs if the
 * island exports it, so timers, animation frames and sockets stop.
 *
 * URLs come from `meta[name=zeyn-islands]`: `{ name: { js, wasm } }`, content hashed files.
 * @module islands
 */

const URLS = (() => {
  try {
    return JSON.parse(document.querySelector('meta[name="zeyn-islands"]')?.content ?? '{}');
  } catch {
    return {};
  }
})();

/** @type {Map<string, Promise<object>>} One module per island name. */
const modules = new Map();
/** @type {Map<Element, object>} Mounted placeholders and their module. */
const mounted = new Map();
const watched = new WeakSet();

/** Fetches and instantiates an island's module once. */
function load(name) {
  let p = modules.get(name);
  if (!p) {
    const urls = URLS[name];
    p = urls
      ? import(urls.js).then(async (m) => {
          await m.default({ module_or_path: urls.wasm });
          return m;
        })
      : Promise.reject(new Error(`island "${name}" is not built`));
    modules.set(name, p);
  }
  return p;
}

/** Loads and mounts one placeholder. */
async function mount(el) {
  if (el.dataset.zeynIslandState) return;
  el.dataset.zeynIslandState = 'loading';
  el.setAttribute('aria-busy', 'true');
  try {
    const m = await load(el.dataset.zeynIsland);
    if (!el.isConnected) {
      delete el.dataset.zeynIslandState;
      return;
    }
    m.mount(el);
    mounted.set(el, m);
    const fallback = el.querySelector(':scope > [data-zeyn-island-fallback]');
    if (fallback) fallback.hidden = true;
    el.dataset.zeynIslandState = 'ready';
  } catch (err) {
    el.dataset.zeynIslandState = 'failed';
    console.warn(`zeyn: island "${el.dataset.zeynIsland}" could not load; its fallback stays`, err);
  } finally {
    el.removeAttribute('aria-busy');
  }
}

const near = new IntersectionObserver(
  (entries) => {
    for (const e of entries) {
      if (e.isIntersecting) {
        near.unobserve(e.target);
        mount(e.target);
      }
    }
  },
  { rootMargin: '200px' },
);

/** Arms one placeholder according to `data-zeyn-load`. */
function watch(el) {
  if (watched.has(el)) return;
  watched.add(el);
  const how = el.dataset.zeynLoad;
  if (how === 'interaction') {
    const go = () => mount(el);
    for (const type of ['pointerenter', 'pointerdown', 'touchstart', 'focusin']) el.addEventListener(type, go, { once: true, passive: true });
  } else if (how === 'idle') {
    (window.requestIdleCallback ?? ((f) => setTimeout(f, 200)))(() => mount(el));
  } else {
    near.observe(el);
  }
}

/** Arms every placeholder in `root` (and `root` itself). */
function scan(root) {
  if (root.matches?.('[data-zeyn-island]')) watch(root);
  for (const el of root.querySelectorAll?.('[data-zeyn-island]') ?? []) watch(el);
}

/** Unmounts islands whose placeholder left the document. */
function sweep() {
  for (const [el, m] of mounted) {
    if (el.isConnected) continue;
    mounted.delete(el);
    try {
      m.unmount?.(el);
    } catch (err) {
      console.warn('zeyn: island unmount failed', err);
    }
  }
}

new MutationObserver((records) => {
  let removed = false;
  for (const r of records) {
    for (const n of r.addedNodes) if (n.nodeType === 1) scan(n);
    removed ||= r.removedNodes.length > 0;
  }
  if (removed && mounted.size) sweep();
}).observe(document.body, { childList: true, subtree: true });

scan(document.body);
