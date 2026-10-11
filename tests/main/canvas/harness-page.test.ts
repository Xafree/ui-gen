import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { buildHarnessPage } from '../../../src/main/canvas/harness-page';

function importMapOf(html: string): { text: string; imports: Record<string, string> } {
  const match = /<script type="importmap">(.*)<\/script>/.exec(html);
  const text = match?.[1] ?? '';
  const parsed: unknown = JSON.parse(text);
  const imports = (parsed as { imports: Record<string, string> }).imports;
  return { text, imports };
}

describe('the harness page', () => {
  it('maps the bare imports of lit to the packages served by the protocol', () => {
    const { imports } = importMapOf(buildHarnessPage(false).html);

    expect(imports).toEqual({
      lit: '/__studio/packages/lit/index.js',
      'lit/': '/__studio/packages/lit/',
      'lit-html': '/__studio/packages/lit-html/lit-html.js',
      'lit-html/': '/__studio/packages/lit-html/',
      'lit-element': '/__studio/packages/lit-element/lit-element.js',
      'lit-element/': '/__studio/packages/lit-element/',
      '@lit/reactive-element': '/__studio/packages/@lit/reactive-element/reactive-element.js',
      '@lit/reactive-element/': '/__studio/packages/@lit/reactive-element/',
    });
  });

  it('allows only its own import map inline, by hash, and keeps the network closed', () => {
    const { html, contentSecurityPolicy } = buildHarnessPage(true);
    const hash = createHash('sha256').update(importMapOf(html).text).digest('base64');

    expect(contentSecurityPolicy).toBe(
      `default-src uigen-canvas:; connect-src 'none'; script-src uigen-canvas: 'sha256-${hash}'; style-src uigen-canvas: 'unsafe-inline'`,
    );
  });

  it('loads the harness script, names the node mark and links the tokens only when asked', () => {
    const withTokens = buildHarnessPage(true).html;
    const withoutTokens = buildHarnessPage(false).html;

    expect(withTokens).toContain('<script type="module" src="/__studio/harness.js"></script>');
    expect(withTokens).toContain('data-node-mark="data-uigen-node"');
    expect(withTokens).toContain('<link rel="stylesheet" href="/tokens/tokens.css">');
    expect(withoutTokens).not.toContain('<link');
  });
});
