import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';
import { chromium } from 'playwright';
import { captureSnapshot } from '../snapshot.mjs';

const fixtureUrl = pathToFileURL(resolve(import.meta.dirname, 'fixtures', 'facts.html')).href;

const elementChildren = (node) => node.children.filter((child) => child.type === 'element');
const isEmpty = (value) => value === undefined || Object.keys(value).length === 0;
const isNone = (value) => value === undefined || value.length === 0;

describe('captureSnapshot facts', () => {
  let browser;
  let page;
  let root;
  let button;
  let plain;
  let spinner;
  let still;

  before(async () => {
    browser = await chromium.launch();
    page = await browser.newPage();
    await page.goto(fixtureUrl);
    ({ root } = await captureSnapshot(page, '#root', { package: 'synthetic' }));
    [button, plain, spinner, still] = elementChildren(root);
  });

  after(async () => {
    await browser?.close();
  });

  // covers: E3-2.1
  it('records the computed flex layout, gaps and paddings in style', () => {
    assert.equal(root.style.display, 'flex');
    assert.equal(root.style.flexDirection, 'column');
    assert.equal(root.style.justifyContent, 'center');
    assert.equal(root.style.rowGap, '8px');
    assert.equal(root.style.columnGap, '8px');
    assert.equal(root.style.paddingTop, '4px');
    assert.equal(root.style.paddingRight, '12px');
    assert.equal(root.style.paddingBottom, '4px');
    assert.equal(root.style.paddingLeft, '12px');
  });

  // covers: E3-2.2
  it('names the custom property behind each var() declaration and keeps the class attribute', () => {
    assert.equal(root.className, 'card');
    assert.equal(button.tag, 'button');
    assert.equal(button.className, 'btn primary');
    assert.equal(button.vars.backgroundColor, '--syn-primary');
    assert.equal(button.vars.borderTopColor, '--syn-border');
    assert.equal(button.style.backgroundColor, 'rgb(10, 20, 30)');
  });

  // covers: E3-2.2
  it('gives no vars to an element whose rules have no var()', () => {
    assert.equal(plain.className, 'plain');
    assert.ok(isEmpty(plain.vars), 'plain element has no vars');
    assert.ok(isEmpty(root.vars), 'container has no vars');
  });

  // covers: E3-2.3
  it('records an infinite rotation as an animation on the animated element', () => {
    assert.equal(spinner.animations.length, 1);
    const [animation] = spinner.animations;
    assert.equal(animation.name, 'spin');
    assert.equal(animation.durationMs, 2000);
    assert.equal(animation.easing, 'linear');
    assert.equal(animation.iterations, 'infinite');
    assert.deepEqual(animation.properties, ['transform']);
  });

  // covers: E3-2.3
  it('gives no animations to elements that are not animated', () => {
    assert.ok(isNone(still.animations), 'still element has no animations');
    assert.ok(isNone(button.animations), 'button has no animations');
    assert.ok(isNone(root.animations), 'container has no animations');
  });
});

describe('captureSnapshot facts limits', () => {
  let browser;
  let page;

  before(async () => {
    browser = await chromium.launch();
    page = await browser.newPage();
  });

  after(async () => {
    await browser?.close();
  });

  // covers: E3-2.4
  it('cuts a class attribute longer than 256 characters and keeps a shorter one whole', async () => {
    const long = 'c'.repeat(300);
    const exact = 'e'.repeat(256);
    await page.setContent(`<div id="root" class="${long}"><div class="${exact}">x</div></div>`);
    const { root } = await captureSnapshot(page, '#root', undefined);

    assert.equal(root.className, long.slice(0, 256));
    const [child] = elementChildren(root);
    assert.equal(child.className, exact);
  });

  // covers: E3-2.4
  it('keeps only the first 16 animations of an element', async () => {
    const names = Array.from({ length: 20 }, (_, index) => `k${index}`);
    const keyframes = names
      .map((name) => `@keyframes ${name} { to { transform: rotate(${names.indexOf(name) + 1}deg); } }`)
      .join('\n');
    await page.setContent(`<style>
      ${keyframes}
      #root { width: 20px; height: 20px; background-color: red;
        animation-name: ${names.join(', ')}; animation-duration: 1s; animation-iteration-count: infinite; }
    </style><div id="root"></div>`);
    const { root } = await captureSnapshot(page, '#root', undefined);

    assert.equal(root.animations.length, 16);
    assert.deepEqual(root.animations.map((animation) => animation.name), names.slice(0, 16));
  });
});
