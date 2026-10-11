import { createHash } from 'node:crypto';
import {
  HARNESS_SCRIPT_FILE,
  PACKAGES_SEGMENT,
  PROJECT_TOKENS_FILE,
  STUDIO_ASSETS_SEGMENT,
} from '../../modules/canvas/domain/canvas-address.js';
import { NODE_MARK_ATTRIBUTE } from '../../modules/canvas/domain/node-mark.js';

export interface HarnessPage {
  readonly html: string;
  readonly contentSecurityPolicy: string;
}

const PACKAGES_PATH = `/${STUDIO_ASSETS_SEGMENT}/${PACKAGES_SEGMENT}`;

const IMPORT_MAP = JSON.stringify({
  imports: {
    lit: `${PACKAGES_PATH}/lit/index.js`,
    'lit/': `${PACKAGES_PATH}/lit/`,
    'lit-html': `${PACKAGES_PATH}/lit-html/lit-html.js`,
    'lit-html/': `${PACKAGES_PATH}/lit-html/`,
    'lit-element': `${PACKAGES_PATH}/lit-element/lit-element.js`,
    'lit-element/': `${PACKAGES_PATH}/lit-element/`,
    '@lit/reactive-element': `${PACKAGES_PATH}/@lit/reactive-element/reactive-element.js`,
    '@lit/reactive-element/': `${PACKAGES_PATH}/@lit/reactive-element/`,
  },
});

const IMPORT_MAP_HASH = createHash('sha256').update(IMPORT_MAP).digest('base64');

export const HARNESS_CONTENT_SECURITY_POLICY = [
  "default-src uigen-canvas:",
  "connect-src 'none'",
  `script-src uigen-canvas: 'sha256-${IMPORT_MAP_HASH}'`,
  "style-src uigen-canvas: 'unsafe-inline'",
].join('; ');

export function buildHarnessPage(withProjectTokens: boolean): HarnessPage {
  const tokens = withProjectTokens ? `\n    <link rel="stylesheet" href="/${PROJECT_TOKENS_FILE}">` : '';
  const html = `<!doctype html>
<html lang="fr" data-node-mark="${NODE_MARK_ATTRIBUTE}">
  <head>
    <meta charset="utf-8">
    <title>Canvas</title>
    <script type="importmap">${IMPORT_MAP}</script>${tokens}
    <script type="module" src="/${STUDIO_ASSETS_SEGMENT}/${HARNESS_SCRIPT_FILE}"></script>
  </head>
  <body></body>
</html>
`;
  return { html, contentSecurityPolicy: HARNESS_CONTENT_SECURITY_POLICY };
}
