import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { LIMITS, captureSnapshot } from '../snapshot.mjs';

const nested = (levels, text) => {
  const inner = text ? 'texte' : '';
  return `<div id="root">${'<div>'.repeat(levels - 1)}${inner}${'</div>'.repeat(levels - 1)}</div>`;
};

describe('captureSnapshot depth', () => {
  let browser;
  let page;

  before(async () => {
    browser = await chromium.launch();
    page = await browser.newPage();
  });

  after(async () => {
    await browser?.close();
  });

  const capture = async (levels, text) => {
    await page.setContent(nested(levels, text));
    return captureSnapshot(page, '#root', undefined);
  };

  it('counts the root as level 1 and accepts a tree of exactly the limit', async () => {
    const snapshot = await capture(LIMITS.maxDepth, false);

    assert.equal(snapshot.root.tag, 'div');
  });

  it('refuses a tree one level over the limit', async () => {
    await assert.rejects(capture(LIMITS.maxDepth + 1, false), /depth/);
  });

  it('counts a text node as a level of its own', async () => {
    const snapshot = await capture(LIMITS.maxDepth - 1, true);

    assert.ok(snapshot.root);
    await assert.rejects(capture(LIMITS.maxDepth, true), /depth/);
  });
});
