import { describe, expect, it } from 'vitest';
import { createSenderGuard } from '../../../src/main/ipc/sender-guard';

const PACKAGED_RENDERER_URL = 'file:///C:/Program%20Files/UI-GEN/resources/app.asar/out/renderer/index.html';
const DEV_SERVER_URL = 'http://localhost:5173';

describe('createSenderGuard with the packaged renderer file', () => {
  const isTrustedSender = createSenderGuard(PACKAGED_RENDERER_URL);

  it('trusts the renderer page itself', () => {
    expect(isTrustedSender(PACKAGED_RENDERER_URL)).toBe(true);
  });

  it('trusts the renderer page with a fragment', () => {
    expect(isTrustedSender(`${PACKAGED_RENDERER_URL}#/editor`)).toBe(true);
  });

  it('trusts the renderer page with a query string', () => {
    expect(isTrustedSender(`${PACKAGED_RENDERER_URL}?theme=dark`)).toBe(true);
  });

  it.each([
    ['another page of the renderer folder', 'file:///C:/Program%20Files/UI-GEN/resources/app.asar/out/renderer/other.html'],
    ['a page elsewhere on the disk', 'file:///C:/Users/emma/Downloads/index.html'],
    ['a page whose path only starts like the renderer', `${PACKAGED_RENDERER_URL}.evil.html`],
    ['the renderer folder itself', 'file:///C:/Program%20Files/UI-GEN/resources/app.asar/out/renderer/'],
    ['the dev server', 'http://localhost:5173/'],
    ['a remote site', 'https://evil.example.com/out/renderer/index.html'],
    ['a javascript: URL', 'javascript:alert(1)'],
    ['a data: URL', 'data:text/html,<script>alert(1)</script>'],
    ['about:blank', 'about:blank'],
    ['an empty string', ''],
    ['a string that is not a URL', 'not a url'],
    ['a bare path', 'C:\\Program Files\\UI-GEN\\resources\\app.asar\\out\\renderer\\index.html'],
  ])('refuses %s', (_label, senderUrl) => {
    expect(isTrustedSender(senderUrl)).toBe(false);
  });
});

describe('createSenderGuard with the development server', () => {
  const isTrustedSender = createSenderGuard(DEV_SERVER_URL);

  it.each([
    ['the server root', 'http://localhost:5173/'],
    ['the server without a trailing slash', 'http://localhost:5173'],
    ['a page of the server', 'http://localhost:5173/index.html'],
    ['a page with a query and a fragment', 'http://localhost:5173/index.html?theme=dark#/editor'],
  ])('trusts %s', (_label, senderUrl) => {
    expect(isTrustedSender(senderUrl)).toBe(true);
  });

  it.each([
    ['another port', 'http://localhost:5174/'],
    ['another scheme', 'https://localhost:5173/'],
    ['another host for the same machine', 'http://127.0.0.1:5173/'],
    ['a host that only starts like the server', 'http://localhost.evil.example.com:5173/'],
    ['a credentials trick', 'http://localhost:5173@evil.example.com/'],
    ['a remote site that mentions the server in its query', 'https://evil.example.com/?next=http://localhost:5173/'],
    ['the packaged renderer file', PACKAGED_RENDERER_URL],
    ['a javascript: URL', 'javascript:alert(1)'],
    ['an empty string', ''],
    ['a string that is not a URL', 'localhost:5173 nope'],
  ])('refuses %s', (_label, senderUrl) => {
    expect(isTrustedSender(senderUrl)).toBe(false);
  });
});

describe('createSenderGuard with an https development server', () => {
  const isTrustedSender = createSenderGuard('https://dev.ui-gen.local:8443/');

  it('trusts a page of the same origin', () => {
    expect(isTrustedSender('https://dev.ui-gen.local:8443/index.html#/home')).toBe(true);
  });

  it('refuses the same host over plain http', () => {
    expect(isTrustedSender('http://dev.ui-gen.local:8443/index.html')).toBe(false);
  });
});
