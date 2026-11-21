// A tiny Markdown subset for text cards and the preview. Everything is escaped FIRST; only our own tags are added,
// so no raw HTML from the clip can ever reach the page.

function esc(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}

/** Inline: `code` (kept literal), **bold**, *italic*. Input is already escaped. */
function inline(s: string): string {
  return s
    .split(/(`[^`\n]+`)/g)
    .map((part, i) => {
      if (i % 2 === 1) return `<code>${part.slice(1, -1)}</code>`;
      return part.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>').replace(/(^|[^*])\*([^*\s][^*\n]*?)\*(?!\*)/g, '$1<em>$2</em>');
    })
    .join('');
}

/** HTML-safe markup for headings 1-3, **b**, *i*, `code`, "-"/"*" lists and line breaks. */
export function renderMd(src: string): string {
  const out: string[] = [];
  let list = false;
  const close = () => {
    if (list) out.push('</ul>');
    list = false;
  };
  for (const raw of src.replace(/\r\n?/g, '\n').split('\n')) {
    const line = esc(raw);
    const h = /^(#{1,3})\s+(.*)$/.exec(line);
    const li = /^\s*[-*]\s+(.*)$/.exec(line);
    if (h) {
      close();
      out.push(`<h${h[1].length}>${inline(h[2])}</h${h[1].length}>`);
    } else if (li) {
      if (!list) out.push('<ul>');
      list = true;
      out.push(`<li>${inline(li[1])}</li>`);
    } else {
      close();
      if (line.trim() !== '') out.push(`<p>${inline(line)}</p>`);
    }
  }
  close();
  return out.join('');
}
