import { readFile } from 'node:fs/promises';
import { basename, join } from 'node:path';
import { prepareComponentModule } from '../../modules/canvas/application/prepare-component-module.js';
import {
  CANVAS_HOST,
  CANVAS_SCHEME,
  HARNESS_PAGE_FILE,
  HARNESS_SCRIPT_FILE,
  PACKAGES_SEGMENT,
  PROJECT_TOKENS_FILE,
  SERVED_PACKAGES,
  STUDIO_ASSETS_SEGMENT,
  readCanvasPath,
} from '../../modules/canvas/domain/canvas-address.js';
import { contentTypeOf, extensionOf } from '../../modules/canvas/domain/content-type.js';
import type { CanvasCompute } from '../../modules/canvas/ports/canvas-compute.js';
import { resolveConfinedFile } from './confined-file.js';
import { buildHarnessPage } from './harness-page.js';

export interface StudioAssets {
  readonly harnessScript: string;
  readonly packagesRoot: string;
}

export interface CanvasHandlerDependencies {
  readonly projectRoot: () => string | null;
  readonly compute: CanvasCompute;
  readonly studio: StudioAssets;
}

const MODULE_CONTENT_TYPE = 'text/javascript; charset=utf-8';
const TSCONFIG_FILE = 'tsconfig.json';
const TOKENS_SEGMENTS = PROJECT_TOKENS_FILE.split('/');

const COMMON_HEADERS: Readonly<Record<string, string>> = {
  'Access-Control-Allow-Origin': '*',
  'Cache-Control': 'no-store',
  'X-Content-Type-Options': 'nosniff',
};

function respond(body: string | Uint8Array | null, status: number, headers: Readonly<Record<string, string>> = {}): Response {
  return new Response(body, { status, headers: { ...COMMON_HEADERS, ...headers } });
}

function refuse(status: number): Response {
  if (status >= 400 && status !== 404) console.warn(`The canvas refused a request (${String(status)})`);
  return respond(null, status);
}

function parseCanvasUrl(value: string): URL | null {
  try {
    const url = new URL(value);
    return url.protocol === `${CANVAS_SCHEME}:` && url.hostname === CANVAS_HOST ? url : null;
  } catch {
    return null;
  }
}

async function readOptionalText(path: string | null): Promise<string | null> {
  return path === null ? null : readFile(path, 'utf8');
}

function packageLocation(segments: readonly string[]): { name: string; file: readonly string[] } | null {
  const scoped = segments[0]?.startsWith('@') === true;
  const name = segments.slice(0, scoped ? 2 : 1).join('/');
  const file = segments.slice(scoped ? 2 : 1);
  return SERVED_PACKAGES.includes(name) && file.length > 0 ? { name, file } : null;
}

async function typeScriptFallback(root: string, segments: readonly string[]): Promise<string | null> {
  const last = segments.at(-1) ?? '';
  if (extensionOf(last) !== '.js') return null;
  return resolveConfinedFile(root, [...segments.slice(0, -1), `${last.slice(0, -3)}.ts`]);
}

export function createCanvasHandler({ projectRoot, compute, studio }: CanvasHandlerDependencies) {
  async function serveHarnessPage(root: string): Promise<Response> {
    const tokens = await resolveConfinedFile(root, TOKENS_SEGMENTS);
    const { html, contentSecurityPolicy } = buildHarnessPage(tokens !== null);
    return respond(html, 200, {
      'Content-Type': 'text/html; charset=utf-8',
      'Content-Security-Policy': contentSecurityPolicy,
    });
  }

  async function serveStudioAsset(root: string, segments: readonly string[]): Promise<Response> {
    const [first = '', ...rest] = segments;
    if (segments.length === 1 && first === HARNESS_PAGE_FILE) return serveHarnessPage(root);
    if (segments.length === 1 && first === HARNESS_SCRIPT_FILE) {
      return respond(await readFile(studio.harnessScript), 200, { 'Content-Type': MODULE_CONTENT_TYPE });
    }
    const location = first === PACKAGES_SEGMENT ? packageLocation(rest) : null;
    if (location === null) return refuse(404);
    const file = await resolveConfinedFile(join(studio.packagesRoot, ...location.name.split('/')), location.file);
    const contentType = file === null ? null : contentTypeOf(file);
    if (file === null || contentType === null) return refuse(404);
    return respond(await readFile(file), 200, { 'Content-Type': contentType });
  }

  async function serveModule(root: string, file: string): Promise<Response> {
    const tsconfig = await readOptionalText(await resolveConfinedFile(root, [TSCONFIG_FILE]));
    const code = await prepareComponentModule(compute, await readFile(file, 'utf8'), tsconfig, basename(file));
    return respond(code, 200, { 'Content-Type': MODULE_CONTENT_TYPE });
  }

  async function serveProjectFile(root: string, segments: readonly string[]): Promise<Response> {
    const file = (await resolveConfinedFile(root, segments)) ?? (await typeScriptFallback(root, segments));
    if (file === null) return refuse(404);
    if (extensionOf(file) === '.ts') return serveModule(root, file);
    const contentType = contentTypeOf(file);
    if (contentType === null) return refuse(415);
    return respond(await readFile(file), 200, { 'Content-Type': contentType });
  }

  return async (request: Request): Promise<Response> => {
    if (request.method !== 'GET') return refuse(405);
    const url = parseCanvasUrl(request.url);
    if (url === null) return refuse(404);
    const segments = readCanvasPath(url.pathname);
    if (segments === null) return refuse(400);
    const root = projectRoot();
    if (root === null) return refuse(404);
    try {
      return segments[0] === STUDIO_ASSETS_SEGMENT
        ? await serveStudioAsset(root, segments.slice(1))
        : await serveProjectFile(root, segments);
    } catch (cause) {
      console.error('A canvas request failed', cause);
      return refuse(500);
    }
  };
}
