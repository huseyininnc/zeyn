/**
 * Streamed text, shared by the assistant (`assistant.js`) and message threads (`thread.js`).
 *
 * Answers arrive as JSON lines (`read`). Model text is shown as a small Markdown subset built
 * from DOM nodes with `textContent` only: no HTML from the model is ever parsed. `writer` keeps
 * the blocks that are finished and renders only the paragraph being written, so a long answer
 * costs the same per frame at its end as at its start and nothing before it moves.
 * @module stream
 */

/** An element with a class and text content. */
export function el(tag, className = '', text = null) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

/**
 * Adds inline Markdown (`code`, **bold**, *italic*, _italic_) to `parent` as text nodes and
 * elements. Anything else stays plain text.
 */
export function inline(text, parent) {
  const re = /(`[^`\n]+`|\*\*[^*\n]+\*\*|\*[^*\s][^*\n]*\*|_[^_\s][^_\n]*_)/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m.index > last) parent.append(text.slice(last, m.index));
    const t = m[0];
    if (t.startsWith('`')) parent.append(el('code', '', t.slice(1, -1)));
    else if (t.startsWith('**')) parent.append(el('strong', '', t.slice(2, -2)));
    else parent.append(el('em', '', t.slice(1, -1)));
    last = m.index + t.length;
  }
  if (last < text.length) parent.append(text.slice(last));
}

/**
 * The safe Markdown subset as a fragment: paragraphs, line breaks, bullet and numbered lists,
 * headings shown as bold paragraphs, and code blocks. Built from `textContent` only.
 */
export function markdown(src) {
  const frag = document.createDocumentFragment();
  const lines = src.replace(/\r/g, '').split('\n');
  let para = null;
  let list = null;
  let code = null;
  const close = () => {
    para = null;
    list = null;
  };
  for (const line of lines) {
    if (code) {
      if (line.trim().startsWith('```')) code = null;
      else code.textContent += (code.textContent ? '\n' : '') + line;
      continue;
    }
    if (line.trim().startsWith('```')) {
      close();
      const pre = el('pre');
      code = el('code');
      pre.append(code);
      frag.append(pre);
      continue;
    }
    const bullet = /^\s*[-*•]\s+(.*)$/.exec(line);
    const numbered = /^\s*\d+[.)]\s+(.*)$/.exec(line);
    if (bullet || numbered) {
      const tag = bullet ? 'UL' : 'OL';
      if (!list || list.tagName !== tag) {
        para = null;
        list = el(tag.toLowerCase());
        frag.append(list);
      }
      const li = el('li');
      inline((bullet ?? numbered)[1], li);
      list.append(li);
      continue;
    }
    if (!line.trim()) {
      close();
      continue;
    }
    const heading = /^#{1,6}\s+(.*)$/.exec(line);
    if (heading) {
      close();
      const p = el('p', 'z-msg-heading');
      const strong = el('strong');
      inline(heading[1], strong);
      p.append(strong);
      frag.append(p);
      continue;
    }
    list = null;
    if (para) para.append(el('br'));
    else {
      para = el('p');
      frag.append(para);
    }
    inline(line, para);
  }
  return frag;
}

/**
 * Writes streamed Markdown into `body`, at most `max` characters. `push` adds a piece and draws
 * once per animation frame: blocks before the last blank line outside a code fence are drawn
 * once and kept, only the rest is drawn again. `text()` is everything so far.
 * @example
 * const w = writer(bubble, 32000);
 * w.push('Hello **there**');
 * w.end();
 */
export function writer(body, max, onPaint = null) {
  let text = '';
  let done = 0;
  let tail = [];
  let frame = 0;
  const paint = () => {
    frame = 0;
    const cut = text.lastIndexOf('\n\n');
    if (cut > done) {
      const fences = (text.slice(done, cut).match(/^[ \t]*```/gm) ?? []).length;
      if (fences % 2 === 0) {
        for (const n of tail) n.remove();
        body.append(markdown(text.slice(done, cut)));
        tail = [];
        done = cut + 2;
      }
    }
    for (const n of tail) n.remove();
    const frag = markdown(text.slice(done));
    tail = [...frag.childNodes];
    body.append(frag);
    onPaint?.();
  };
  return {
    push(piece) {
      if (text.length >= max || typeof piece !== 'string') return;
      text += piece.slice(0, max - text.length);
      frame ||= requestAnimationFrame(paint);
    },
    end() {
      if (frame) cancelAnimationFrame(frame);
      paint();
    },
    text: () => text,
  };
}

/** Reads a response body as JSON lines and calls `handle` with each parsed line. */
export async function read(res, handle) {
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = '';
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    let nl;
    while ((nl = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, nl).trim();
      buf = buf.slice(nl + 1);
      if (!line) continue;
      let event;
      try {
        event = JSON.parse(line);
      } catch {
        continue;
      }
      handle(event);
    }
  }
}
