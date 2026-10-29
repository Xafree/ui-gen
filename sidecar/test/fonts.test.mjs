import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { chromium } from 'playwright';
import { captureSnapshot } from '../snapshot.mjs';

const fixtureUrl = (name) => pathToFileURL(resolve(import.meta.dirname, 'fixtures', name)).href;

const collect = (node) => [node, ...(node.children ?? []).flatMap(collect)];
const BOX_KEYS = ['x', 'y', 'width', 'height'];

describe('text fidelity in the snapshot', () => {
  let browser;
  let page;

  before(async () => {
    browser = await chromium.launch();
    page = await browser.newPage();
  });

  after(async () => {
    await browser?.close();
  });

  // covers: P1
  it('records the font Chromium really used for a text', async () => {
    await page.goto(fixtureUrl('fonts.html'));
    const snapshot = await captureSnapshot(page, '#root', { package: 'synthetic' });

    const text = snapshot.root.children.find((child) => child.type === 'text');
    assert.ok(text, 'text child');
    assert.equal(typeof text.style.platformFont, 'string');
    assert.notEqual(text.style.platformFont.trim(), '', 'platformFont is not empty');
    assert.notEqual(text.style.platformFont, 'NoSuchFont');
    assert.ok(!text.style.platformFont.includes('NoSuchFont'));
  });

  // covers: P1
  it('records the line box: the computed line height, centred on the content box', async () => {
    await page.goto(fixtureUrl('fonts.html'));
    const snapshot = await captureSnapshot(page, '#root', { package: 'synthetic' });

    const text = snapshot.root.children.find((child) => child.type === 'text');
    assert.ok(text.lineBox, 'lineBox is present');
    for (const key of BOX_KEYS) {
      assert.ok(Number.isFinite(text.lineBox[key]), `lineBox.${key} is finite`);
    }
    assert.ok(Math.abs(text.lineBox.height - 24) < 0.01, `lineBox.height is 24, got ${text.lineBox.height}`);
    assert.ok(Math.abs(text.lineBox.x - text.box.x) < 0.01, 'lineBox starts where box starts');
    assert.ok(Math.abs(text.lineBox.width - text.box.width) < 0.01, 'lineBox is as wide as box');
    const lineCentre = text.lineBox.y + text.lineBox.height / 2;
    const boxCentre = text.box.y + text.box.height / 2;
    assert.ok(Math.abs(lineCentre - boxCentre) < 0.01, `lineBox is centred on box (${lineCentre} vs ${boxCentre})`);
  });

  // covers: P1
  it('records platformFont and lineBox on texts only, not on elements or svg', async () => {
    await page.goto(fixtureUrl('button.html'));
    const snapshot = await captureSnapshot(page, '#root', { package: 'synthetic' });

    const nodes = collect(snapshot.root);
    const texts = nodes.filter((node) => node.type === 'text');
    assert.ok(texts.length > 0);
    for (const text of texts) {
      assert.ok(text.style.platformFont, 'every text has platformFont');
      assert.ok(text.lineBox, 'every text has lineBox');
    }
    for (const node of nodes.filter((candidate) => candidate.type !== 'text')) {
      assert.equal(node.lineBox, undefined, `${node.type} has no lineBox`);
      assert.equal(node.style?.platformFont, undefined, `${node.type} has no platformFont`);
    }
  });
});
