import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { captureSnapshot } from '../snapshot.mjs';

const HTML = `<!doctype html><html><head><style>
body { margin: 0; }
#root { position: relative; width: 200px; height: 100px; filter: blur(2px); z-index: 3; }
#ring::after { content: ''; position: absolute; left: 10px; top: 20px; width: 30px; height: 4px; background: rgb(1, 2, 3); transform: scaleX(0.5); }
#ring::before { content: ''; position: absolute; left: 0; top: 0; width: 5px; height: 5px; }
#flow::before { content: '*'; color: red; }
#icon { color: oklch(0.6 0.2 30); transform: rotate(180deg); width: 24px; height: 24px; }
#icon path { stroke-width: 3px; }
#pressed { transform: scale(0.5); font-size: 20px; line-height: 40px; }
</style></head><body><div id="root">
<div id="ring">x</div><div id="flow">y</div>
<svg id="icon" viewBox="0 0 24 24"><path d="M0 0L24 24" stroke="currentColor"/></svg>
<div id="pressed">z</div>
</div></body></html>`;

const elements = (node) => node.children.filter((child) => child.type === 'element');

describe('captureSnapshot effects', () => {
  let browser;
  let root;

  before(async () => {
    browser = await chromium.launch();
    const page = await browser.newPage();
    await page.setContent(HTML);
    ({ root } = await captureSnapshot(page, '#root', { package: 'synthetic' }));
  });

  after(async () => {
    await browser.close();
  });

  it('records the computed effects of an element', () => {
    assert.equal(root.style.filter, 'blur(2px)');
    assert.equal(root.style.zIndex, '3');
    assert.equal(root.style.mixBlendMode, 'normal');
  });

  it('turns a positioned pseudo-element that paints into a child, and skips one that paints nothing', () => {
    const [ring] = elements(root);
    const pseudos = elements(ring);
    assert.deepEqual(pseudos.map((pseudo) => pseudo.tag), ['::after']);
    assert.deepEqual(pseudos[0].box, { x: 17.5, y: 20, width: 15, height: 4 });
    assert.equal(pseudos[0].style.backgroundColor, 'rgb(1, 2, 3)');
  });

  it('names a pseudo-element that cannot become a layer', () => {
    const [, flow] = elements(root);
    assert.match(flow.style.uigenLostPseudo, /::before "\*" is in the flow/);
  });

  it('gives an svg sRGB colours, its css presentation and its rotation', () => {
    const icon = root.children.find((child) => child.type === 'svg');
    assert.equal(icon.rotation, 180);
    assert.doesNotMatch(icon.markup, /oklch/);
    assert.match(icon.markup, /stroke="rgb\(/);
    assert.match(icon.markup, /stroke-width="3px"/);
  });

  it('scales the text of a scaled element', () => {
    const pressed = elements(root).at(-1);
    const [text] = pressed.children;
    assert.equal(text.style.fontSize, '10px');
    assert.equal(text.style.lineHeight, '20px');
  });
});
