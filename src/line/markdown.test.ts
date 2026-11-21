import { describe, expect, it } from 'vitest';
import { renderMd } from './markdown';

describe('renderMd', () => {
  it('markdown_escapes_html', () => {
    const a = renderMd('<script>x</script>');
    expect(a).toContain('&lt;script&gt;');
    expect(a).not.toContain('<script');
    expect(renderMd('<img src=x onerror=1>')).not.toContain('<img');
    const b = renderMd('# Başlık\n**kalın** ve *eğik*\n- a\n- b');
    expect(b).toContain('<h1>');
    expect(b).toContain('<strong>');
    expect(b).toContain('<em>');
    expect(b).toContain('<ul>');
    expect(b.match(/<li>/g)?.length).toBe(2);
  });

  it('code_is_not_formatted_inside', () => {
    const c = renderMd('`a *b* <i>`');
    expect(c).toContain('<code>a *b* &lt;i&gt;</code>');
  });
});
