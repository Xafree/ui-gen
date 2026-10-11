import { readFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { expect, type ElectronApplication, type Locator, type Page, type TestInfo } from '@playwright/test';

export type Theme = 'light' | 'dark';

export interface ReferenceWindow {
  readonly id: number;
  readonly page: Page;
}

interface ImageSize {
  readonly width: number;
  readonly height: number;
}

interface PixelComparison {
  readonly reference: ImageSize;
  readonly studio: ImageSize;
  readonly differing: number;
  readonly diffPng: string;
}

const DEV_ROOT = process.cwd();
const MOCKUPS = resolve(DEV_ROOT, '..', 'maquettes');
const REFERENCE_PARTITION = 'ui-gen-parity-reference';
const CONTENT_SIZE: ImageSize = { width: 1440, height: 900 };
const MASK_COLOR = '#FF00FF';
const SETTLED_EXPRESSION = `(() => {
  const gsap = globalThis.gsap;
  const tweening = gsap ? gsap.globalTimeline.getChildren(true, true, false).some((tween) => tween.isActive()) : false;
  return document.getAnimations().length === 0 && !tweening;
})()`;

export function mockupPath(name: string): string {
  return join(MOCKUPS, name);
}

async function fontFaceSheet(): Promise<string> {
  const nodeModules = pathToFileURL(join(DEV_ROOT, 'node_modules')).href;
  const studioFonts = await readFile(join(DEV_ROOT, 'src', 'renderer', 'styles', 'fonts.css'), 'utf8');
  const sheet = studioFonts.replaceAll('url(../../../node_modules', `url(${nodeModules}`);
  expect(sheet, 'fonts.css must point at node_modules/@fontsource-variable').toContain(`${nodeModules}/@fontsource-variable/`);
  return sheet;
}

export async function openReferenceWindow(app: ElectronApplication): Promise<ReferenceWindow> {
  const sheet = await fontFaceSheet();
  const gsapUrl = pathToFileURL(join(DEV_ROOT, 'node_modules', 'gsap', 'dist', 'gsap.min.js')).href;
  const created = app.waitForEvent('window');
  const id = await app.evaluate(
    ({ BrowserWindow, net, session }, options) => {
      const fontsUrl = 'https://fonts.googleapis.com/css2';
      const gsapCdnUrl = 'https://cdnjs.cloudflare.com/ajax/libs/gsap/3.12.5/gsap.min.js';
      const isServed = (url: string): boolean => url.startsWith(fontsUrl) || url === gsapCdnUrl;
      const isLocal = (url: string): boolean => url.startsWith('file:') || url.startsWith('data:');
      const reference = session.fromPartition(options.partition);
      reference.webRequest.onBeforeRequest((details, callback) => {
        callback({ cancel: !isLocal(details.url) && !isServed(details.url) });
      });
      if (!reference.protocol.isProtocolHandled('https')) {
        reference.protocol.handle('https', (request) => {
          if (request.url.startsWith(fontsUrl)) {
            return new Response(options.sheet, { headers: { 'content-type': 'text/css' } });
          }
          if (request.url === gsapCdnUrl) return net.fetch(options.gsapUrl);
          return new Response(null, { status: 404 });
        });
      }
      const window = new BrowserWindow({
        width: options.size.width,
        height: options.size.height,
        useContentSize: true,
        show: true,
        webPreferences: { partition: options.partition, sandbox: true, contextIsolation: true, nodeIntegration: false },
      });
      void window.loadURL('about:blank');
      return window.id;
    },
    { partition: REFERENCE_PARTITION, sheet, gsapUrl, size: CONTENT_SIZE },
  );
  const page = await created;
  return { id, page };
}

export async function showMockup(app: ElectronApplication, reference: ReferenceWindow, mockup: string, theme: Theme): Promise<void> {
  const file = mockupPath(mockup);
  const load = (): Promise<void> =>
    app.evaluate(async ({ BrowserWindow }, target) => {
      const window = BrowserWindow.fromId(target.id);
      if (window === null) throw new Error(`reference window ${String(target.id)} is gone`);
      await window.loadFile(target.file);
    }, { id: reference.id, file });
  await load();
  await reference.page.evaluate((chosen) => {
    localStorage.setItem('uigen-theme', chosen);
  }, theme);
  await load();
  expect(await reference.page.evaluate('document.documentElement.getAttribute("data-theme")')).toBe(theme);
  expect(
    await app.evaluate(({ BrowserWindow }, id) => BrowserWindow.fromId(id)?.getContentSize(), reference.id),
  ).toEqual([CONTENT_SIZE.width, CONTENT_SIZE.height]);
}

export async function closeReferenceWindow(app: ElectronApplication, reference: ReferenceWindow): Promise<void> {
  await app.evaluate(({ BrowserWindow }, id) => {
    BrowserWindow.fromId(id)?.destroy();
  }, reference.id);
}

export async function expectFontsLoaded(page: Page): Promise<void> {
  await page.evaluate('document.fonts.ready.then(() => true)');
  const families: unknown = await page.evaluate(
    '[...document.fonts].filter((face) => face.status === "loaded").map((face) => face.family.replaceAll(\'"\', ""))',
  );
  expect(families).toEqual(expect.arrayContaining(['Instrument Sans', 'Geist Mono']));
}

export async function settledCapture(page: Page, mask: Locator): Promise<Buffer> {
  await page.bringToFront();
  await page.evaluate('document.fonts.ready.then(() => true)');
  await expect.poll(() => page.evaluate(SETTLED_EXPRESSION), { timeout: 15_000 }).toBe(true);
  let previous: Buffer = Buffer.alloc(0);
  let latest: Buffer = Buffer.alloc(0);
  await expect(async () => {
    previous = latest;
    latest = await page.screenshot({ mask: [mask], maskColor: MASK_COLOR, caret: 'hide', animations: 'disabled' });
    expect(latest.equals(previous)).toBe(true);
  }).toPass({ intervals: [250], timeout: 15_000 });
  return latest;
}

async function comparePixels(app: ElectronApplication, reference: Buffer, studio: Buffer): Promise<PixelComparison> {
  return app.evaluate(
    ({ nativeImage }, images) => {
      const expected = nativeImage.createFromDataURL(`data:image/png;base64,${images.reference}`);
      const actual = nativeImage.createFromDataURL(`data:image/png;base64,${images.studio}`);
      const referenceSize = expected.getSize();
      const studioSize = actual.getSize();
      if (referenceSize.width !== studioSize.width || referenceSize.height !== studioSize.height) {
        return { reference: referenceSize, studio: studioSize, differing: Number.NaN, diffPng: '' };
      }
      const left = expected.toBitmap();
      const right = actual.toBitmap();
      const diff = Buffer.alloc(left.length);
      let differing = 0;
      for (let offset = 0; offset < left.length; offset += 4) {
        const same =
          left[offset] === right[offset] &&
          left[offset + 1] === right[offset + 1] &&
          left[offset + 2] === right[offset + 2] &&
          left[offset + 3] === right[offset + 3];
        if (same) {
          const faded = 200 + Math.round(((left[offset] ?? 0) + (left[offset + 1] ?? 0) + (left[offset + 2] ?? 0)) / 15);
          diff.fill(Math.min(faded, 255), offset, offset + 3);
        } else {
          differing += 1;
          diff[offset] = 0;
          diff[offset + 1] = 0;
          diff[offset + 2] = 255;
        }
        diff[offset + 3] = 255;
      }
      const diffPng = differing === 0 ? '' : nativeImage.createFromBitmap(diff, referenceSize).toPNG().toString('base64');
      return { reference: referenceSize, studio: studioSize, differing, diffPng };
    },
    { reference: reference.toString('base64'), studio: studio.toString('base64') },
  );
}

export async function expectIdenticalCaptures(
  app: ElectronApplication,
  testInfo: TestInfo,
  name: string,
  reference: Buffer,
  studio: Buffer,
): Promise<void> {
  const comparison = await comparePixels(app, reference, studio);
  const identical = comparison.differing === 0;
  if (!identical) {
    await testInfo.attach(`${name}-mockup.png`, { body: reference, contentType: 'image/png' });
    await testInfo.attach(`${name}-studio.png`, { body: studio, contentType: 'image/png' });
    if (comparison.diffPng !== '') {
      await testInfo.attach(`${name}-diff.png`, { body: Buffer.from(comparison.diffPng, 'base64'), contentType: 'image/png' });
    }
  }
  expect(comparison.studio, `${name}: studio capture size`).toEqual(comparison.reference);
  expect(comparison.differing, `${name}: ${String(comparison.differing)} pixels differ from the mockup`).toBe(0);
}
