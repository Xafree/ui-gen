import { describe, expect, it } from 'vitest';
import { createSenderGuard } from '../../../src/main/ipc/sender-guard';

const PACKAGED_RENDERER_URL = 'file:///C:/Program%20Files/UI-GEN/resources/app.asar/out/renderer/index.html';
const DEV_SERVER_URL = 'http://localhost:5173';

describe('createSenderGuard configuration', () => {
  it('fails at once when the renderer URL cannot be parsed', () => {
    expect(() => createSenderGuard('not a url')).toThrow(TypeError);
  });

  it.each([
    ['a wss: URL', 'wss://localhost:5173/'],
    ['about:blank', 'about:blank'],
    ['a custom protocol', 'app://studio/index.html'],
  ])('fails at once for %s, which is neither a file nor a web page', (_label, rendererUrl) => {
    expect(() => createSenderGuard(rendererUrl)).toThrow(TypeError);
  });

  it('fails at once when the renderer file is not on a Windows path', () => {
    expect(() => createSenderGuard('file:///opt/ui-gen/out/renderer/index.html')).toThrow(TypeError);
  });
});

describe('createSenderGuard with the packaged renderer file', () => {
  const isTrustedSender = createSenderGuard(PACKAGED_RENDERER_URL);

  it('refuses the same path served by another host', () => {
    expect(
      isTrustedSender('file://share.example.com/C:/Program%20Files/UI-GEN/resources/app.asar/out/renderer/index.html'),
    ).toBe(false);
  });

  it('refuses the renderer page reached through a blob: URL', () => {
    expect(isTrustedSender(`blob:${PACKAGED_RENDERER_URL}`)).toBe(false);
  });

  it('refuses the same path on another drive', () => {
    expect(isTrustedSender(PACKAGED_RENDERER_URL.replace('file:///C:', 'file:///D:'))).toBe(false);
  });

  it('refuses a path that hides a separator behind a percent escape', () => {
    expect(isTrustedSender(PACKAGED_RENDERER_URL.replace('/out/renderer/', '/out%2Frenderer/'))).toBe(false);
    expect(isTrustedSender(PACKAGED_RENDERER_URL.replace('/out/renderer/', '/out%5Crenderer/'))).toBe(false);
  });

  it('refuses a path with a malformed percent escape instead of throwing', () => {
    expect(isTrustedSender(PACKAGED_RENDERER_URL.replace('index.html', 'index%E0%A4%A.html'))).toBe(false);
  });

  it('refuses a file URL that is not on a Windows path', () => {
    expect(isTrustedSender('file:///opt/ui-gen/out/renderer/index.html')).toBe(false);
  });

  it('compares the path without case, as Windows does', () => {
    expect(isTrustedSender(PACKAGED_RENDERER_URL.replace('index.html', 'INDEX.HTML'))).toBe(true);
  });
});

describe('createSenderGuard with a renderer path written with a lowercase drive letter', () => {
  const lowercaseDriveUrl = PACKAGED_RENDERER_URL.replace('file:///C:', 'file:///c:');
  const isTrustedSender = createSenderGuard(lowercaseDriveUrl);

  it('trusts the page that Chromium reports with an uppercase drive letter', () => {
    expect(isTrustedSender(PACKAGED_RENDERER_URL)).toBe(true);
  });

  it('trusts the page reported with the lowercase drive letter too', () => {
    expect(isTrustedSender(lowercaseDriveUrl)).toBe(true);
  });

  it('still refuses another page of the same folder', () => {
    expect(isTrustedSender(PACKAGED_RENDERER_URL.replace('index.html', 'other.html'))).toBe(false);
  });
});

describe('createSenderGuard with a path that Node and Chromium do not escape the same way', () => {
  const nodeForm = 'file:///C:/Users/EMMAN%7E1/UI-GEN%20%5Bbeta%5D/resources/app.asar/out/renderer/index.html';
  const chromiumForm = 'file:///C:/Users/EMMAN~1/UI-GEN%20[beta]/resources/app.asar/out/renderer/index.html';

  it('trusts the Chromium form when the renderer URL is written the Node way', () => {
    expect(createSenderGuard(nodeForm)(chromiumForm)).toBe(true);
  });

  it('trusts the Node form when the renderer URL is written the Chromium way', () => {
    expect(createSenderGuard(chromiumForm)(nodeForm)).toBe(true);
  });

  it('trusts a path with accents whichever way they are written', () => {
    const isTrustedSender = createSenderGuard(
      'file:///C:/Users/%C3%89mile/UI-GEN/resources/app.asar/out/renderer/index.html',
    );

    expect(isTrustedSender('file:///C:/Users/\u00c9mile/UI-GEN/resources/app.asar/out/renderer/index.html')).toBe(true);
  });
});

describe('createSenderGuard with the development server', () => {
  const isTrustedSender = createSenderGuard(DEV_SERVER_URL);

  it('refuses a blob: URL created by the development server origin', () => {
    expect(isTrustedSender('blob:http://localhost:5173/6f1c2c1e-6b0a-4f5e-9d54-0a8e2f3c9d11')).toBe(false);
  });

  it('refuses a WebSocket URL of the development server', () => {
    expect(isTrustedSender('wss://localhost:5173/')).toBe(false);
  });
});
