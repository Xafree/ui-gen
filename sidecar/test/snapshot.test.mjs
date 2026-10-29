import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { chromium } from 'playwright';
import { captureSnapshot } from '../snapshot.mjs';

const fixtureUrl = pathToFileURL(resolve(import.meta.dirname, 'fixtures', 'button.html')).href;

const collect = (node) => [node, ...(node.children ?? []).flatMap(collect)];

describe('captureSnapshot', () => {
  let browser;
  let page;

  before(async () => {
    browser = await chromium.launch();
    page = await browser.newPage();
    await page.goto(fixtureUrl);
  });

  after(async () => {
    await browser?.close();
  });

  // covers: S1
  it('captures a v1 snapshot of the synthetic button', async () => {
    const source = { package: 'synthetic' };
    const snapshot = await captureSnapshot(page, '#root', source);

    assert.equal(snapshot.version, 1);
    assert.deepEqual(snapshot.source, source);
    assert.ok(snapshot.viewport);
    assert.ok(snapshot.viewport.width > 0);
    assert.ok(snapshot.viewport.height > 0);
    assert.ok(snapshot.viewport.deviceScaleFactor > 0);

    const { root } = snapshot;
    assert.equal(root.type, 'element');
    assert.equal(root.tag, 'button');
    for (const key of ['x', 'y', 'width', 'height']) {
      assert.ok(Number.isFinite(root.box[key]), `box.${key} is finite`);
    }
    assert.ok(root.box.width > 0);
    assert.ok(root.box.height > 0);

    assert.equal(root.style.backgroundColor, 'rgb(25, 118, 210)');
    assert.equal(root.style.borderTopWidth, '1px');
    assert.equal(root.style.borderTopLeftRadius, '4px');
    assert.notEqual(root.style.boxShadow, 'none');

    const text = root.children.find((child) => child.type === 'text' && child.text.trim() === 'Bouton');
    assert.ok(text, 'text child "Bouton"');
    assert.ok(text.style.color);
    assert.ok(text.style.fontFamily);
    assert.ok(text.style.fontSize);
    assert.ok(text.style.fontWeight);
    assert.ok(text.box.width > 0);

    const svg = root.children.find((child) => child.type === 'svg');
    assert.ok(svg, 'svg child');
    assert.ok(svg.markup.startsWith('<svg'));
    assert.ok(svg.box.width > 0);

    const everything = JSON.stringify(collect(root));
    assert.ok(!everything.includes('Secret'), 'hidden span is absent');
    assert.ok(!collect(root).some((node) => node.type === 'element' && node.tag === 'span'));
  });

  // covers: S2
  it('captures the hover style when the element is hovered', async () => {
    await page.hover('#root');
    const snapshot = await captureSnapshot(page, '#root', { package: 'synthetic', state: 'hover' });

    assert.equal(snapshot.root.style.backgroundColor, 'rgb(21, 101, 192)');
  });
});
