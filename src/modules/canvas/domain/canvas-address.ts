export const CANVAS_SCHEME = 'uigen-canvas';
export const CANVAS_HOST = 'canvas';
export const CANVAS_ORIGIN = `${CANVAS_SCHEME}://${CANVAS_HOST}`;
export const STUDIO_ASSETS_SEGMENT = '__studio';
export const HARNESS_PAGE_FILE = 'harness.html';
export const HARNESS_SCRIPT_FILE = 'harness.js';
export const PACKAGES_SEGMENT = 'packages';
export const PROJECT_TOKENS_FILE = 'tokens/tokens.css';
export const HARNESS_PAGE_URL = `${CANVAS_ORIGIN}/${STUDIO_ASSETS_SEGMENT}/${HARNESS_PAGE_FILE}`;

export const SERVED_PACKAGES: readonly string[] = ['lit', 'lit-html', 'lit-element', '@lit/reactive-element'];

function hasForbiddenCharacter(segment: string): boolean {
  for (const character of segment) {
    if ('\\/:'.includes(character) || character.charCodeAt(0) < 32) return true;
  }
  return false;
}

export function readCanvasPath(pathname: string): string[] | null {
  if (!pathname.startsWith('/')) return null;
  const segments: string[] = [];
  for (const raw of pathname.slice(1).split('/')) {
    let segment: string;
    try {
      segment = decodeURIComponent(raw);
    } catch {
      return null;
    }
    if (segment === '' || segment.startsWith('.') || hasForbiddenCharacter(segment)) return null;
    segments.push(segment);
  }
  return segments;
}
