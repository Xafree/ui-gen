import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, symlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { closeBrowser } from '../browser.mjs';
import { render } from '../render.mjs';
import { makeTempDir } from './helpers/review-project.mjs';

const MAX_STYLESHEET_BYTES = 8 * 1024 * 1024;
const BLUE = 'rgb(51, 102, 153)';
const RED = 'rgb(255, 0, 0)';

const CARD_SOURCE = `import { createElement } from 'react';
export function Card() { return createElement('div', { className: 'sty-card' }, 'card'); }
`;

const manifestOf = (name, extra = {}) =>
  `${JSON.stringify({ name, version: '1.0.0', type: 'module', main: 'index.js', peerDependencies: { react: '>=18' }, ...extra })}\n`;

const cardCss = (background, radius) => `.sty-card { background: ${background}; border-radius: ${radius}; }\n`;

const collect = (node) => [node, ...(node.children ?? []).flatMap(collect)];

const cardOf = (snapshot) => {
  const card = collect(snapshot.root).find(
    (node) => node.type === 'element' && (node.children ?? []).some((child) => child.type === 'text' && child.text === 'card'),
  );
  assert.ok(card, 'the card element is in the snapshot');
  return card;
};

const textsOf = (snapshot) => collect(snapshot.root).filter((node) => node.type === 'text').map((node) => node.text);

const renderOf = (dir, name, component, theme = 'light') =>
  render({ dir, package: name, component, props: {}, state: 'default', theme });

describe('library stylesheets in component renders', () => {
  const cleanups = [];

  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });

  after(closeBrowser);

  const project = async (packages) => {
    const made = await makeTempDir();
    cleanups.push(made.remove);
    const modules = path.join(made.dir, '.uigen', 'sources', 'node_modules');
    await mkdir(modules, { recursive: true });
    await writeFile(path.join(modules, '..', 'package.json'), `${JSON.stringify({ name: 'uigen-sources', private: true })}\n`);
    for (const [name, files] of Object.entries(packages)) {
      for (const [file, contents] of Object.entries(files)) {
        const target = path.join(modules, ...name.split('/'), ...file.split('/'));
        await mkdir(path.dirname(target), { recursive: true });
        await writeFile(target, contents);
      }
    }
    return { dir: made.dir, modules };
  };

  // covers: STY-1
  it('applies the stylesheet named by the "style" field of the package', async () => {
    const { dir } = await project({
      'plain-css-lib': {
        'package.json': manifestOf('plain-css-lib', { style: './dist/index.css' }),
        'index.js': CARD_SOURCE,
        'dist/index.css': cardCss('#336699', '12px'),
      },
    });

    const { snapshot } = await renderOf(dir, 'plain-css-lib', 'Card');

    const card = cardOf(snapshot);
    assert.equal(card.style.backgroundColor, BLUE);
    assert.equal(card.style.borderTopLeftRadius, '12px');
  });

  it('measures a lone rendered element on its own box, without the bootstrap div', async () => {
    const { dir } = await project({
      'plain-css-lib': {
        'package.json': manifestOf('plain-css-lib', { style: './dist/index.css' }),
        'index.js': CARD_SOURCE,
        'dist/index.css': cardCss('#336699', '12px'),
      },
    });

    const { snapshot } = await renderOf(dir, 'plain-css-lib', 'Card');

    assert.equal(snapshot.root, cardOf(snapshot), 'the card is the root of the snapshot');
  });

  // covers: STY-2
  it('applies the stylesheet of a sibling styles package the library depends on', async () => {
    const { dir } = await project({
      '@sty/react': {
        'package.json': manifestOf('@sty/react', { dependencies: { '@sty/styles': '1.0.0' } }),
        'index.js': CARD_SOURCE,
      },
      '@sty/styles': {
        'package.json': manifestOf('@sty/styles', { peerDependencies: {}, style: './dist/index.css' }),
        'dist/index.css': cardCss('#336699', '12px'),
      },
    });

    const { snapshot } = await renderOf(dir, '@sty/react', 'Card');

    const card = cardOf(snapshot);
    assert.equal(card.style.backgroundColor, BLUE);
    assert.equal(card.style.borderTopLeftRadius, '12px');
  });

  // covers: STY-3
  it('uses the compiled min.css of the package when the "style" entry holds unresolved @import', async () => {
    const { dir } = await project({
      'tailwind-lib': {
        'package.json': manifestOf('tailwind-lib', { style: './dist/index.css' }),
        'index.js': CARD_SOURCE,
        'dist/index.css': `@import "tailwindcss";\n@import "./components/card.css";\n${cardCss('#ff0000', '1px')}`,
        'dist/tailwind-lib.min.css': cardCss('#336699', '12px'),
      },
    });

    const { snapshot } = await renderOf(dir, 'tailwind-lib', 'Card');

    const card = cardOf(snapshot);
    assert.equal(card.style.backgroundColor, BLUE);
    assert.equal(card.style.borderTopLeftRadius, '12px');
  });

  // covers: STY-4
  it('sets the dark class and data-theme on the html element of a dark render only', async () => {
    const { dir } = await project({
      'theme-lib': {
        'package.json': manifestOf('theme-lib'),
        'index.js': `import { createElement } from 'react';
const STYLE = '.dark .tone { background: #000000; } .tone { background: #ffffff; }';
export function Tone() {
  const root = document.documentElement;
  const state = 'html:' + root.className + '; theme:' + root.getAttribute('data-theme');
  return createElement('div', { className: 'tone' }, createElement('style', null, STYLE), state);
}
`,
      },
    });

    const dark = (await renderOf(dir, 'theme-lib', 'Tone', 'dark')).snapshot;
    const light = (await renderOf(dir, 'theme-lib', 'Tone', 'light')).snapshot;

    const darkState = textsOf(dark).find((text) => text.startsWith('html:'));
    const lightState = textsOf(light).find((text) => text.startsWith('html:'));
    assert.match(darkState ?? '', /^html:(?:.*\s)?dark(?:\s.*)?; theme:dark$/);
    assert.ok(lightState !== undefined && !lightState.includes('dark'), lightState);
    const toneOf = (snapshot) =>
      collect(snapshot.root).find((node) => node.type === 'element' && node.children.some((child) => child.type === 'text' && child.text.startsWith('html:')));
    assert.equal(toneOf(dark)?.style.backgroundColor, 'rgb(0, 0, 0)');
    assert.equal(toneOf(light)?.style.backgroundColor, 'rgb(255, 255, 255)');
  });

  describe('stylesheets outside the package', () => {
    const dependingOnBlueStyles = (ownStyle) => ({
      '@sty/styles': {
        'package.json': manifestOf('@sty/styles', { peerDependencies: {}, style: './dist/index.css' }),
        'dist/index.css': '.sty-card { background: #336699; }\n',
      },
      '@sty/react': {
        'package.json': manifestOf('@sty/react', { dependencies: { '@sty/styles': '1.0.0' }, style: ownStyle }),
        'index.js': CARD_SOURCE,
      },
    });

    // covers: STY-5
    it('does not read a "style" path that escapes the package through ..', async () => {
      const { dir } = await project({
        ...dependingOnBlueStyles('../leak/leak.css'),
        leak: { 'leak.css': '.sty-card { border-radius: 99px; }\n' },
      });

      const { snapshot } = await renderOf(dir, '@sty/react', 'Card');

      const card = cardOf(snapshot);
      assert.equal(card.style.backgroundColor, BLUE);
      assert.notEqual(card.style.borderTopLeftRadius, '99px');
    });

    // covers: STY-5
    it('does not read a stylesheet reached through a symlink that leaves the sources', async () => {
      const outside = await makeTempDir('uigen-outside-');
      cleanups.push(outside.remove);
      await writeFile(path.join(outside.dir, 'index.css'), '.sty-card { border-radius: 99px; }\n');
      const { dir, modules } = await project(dependingOnBlueStyles('./linked/index.css'));
      await symlink(outside.dir, path.join(modules, '@sty', 'react', 'linked'), 'junction');

      const { snapshot } = await renderOf(dir, '@sty/react', 'Card');

      const card = cardOf(snapshot);
      assert.equal(card.style.backgroundColor, BLUE);
      assert.notEqual(card.style.borderTopLeftRadius, '99px');
    });
  });

  // covers: STY-5
  it('ignores a stylesheet over 8 MiB and says so in a note', async () => {
    const { dir } = await project({
      'heavy-css-lib': {
        'package.json': manifestOf('heavy-css-lib', { style: './dist/index.css' }),
        'index.js': CARD_SOURCE,
        'dist/index.css': `${cardCss('#ff0000', '1px')}${' '.repeat(MAX_STYLESHEET_BYTES + 1)}`,
      },
    });

    const result = await renderOf(dir, 'heavy-css-lib', 'Card');

    assert.notEqual(cardOf(result.snapshot).style.backgroundColor, RED);
    assert.equal(typeof result.note, 'string');
    assert.match(result.note, /stylesheet/i);
  });

  // covers: STY-6
  it('renders a library without any stylesheet as before, with no note', async () => {
    const { dir } = await project({
      'css-in-js-lib': {
        'package.json': manifestOf('css-in-js-lib'),
        'index.js': `import { createElement } from 'react';
export function Card() {
  return createElement('div', { className: 'sty-card' }, createElement('style', null, '.sty-card { background: #336699; }'), 'card');
}
`,
      },
    });

    const result = await renderOf(dir, 'css-in-js-lib', 'Card');

    assert.equal(cardOf(result.snapshot).style.backgroundColor, BLUE);
    assert.equal(result.note, undefined);
  });
});
