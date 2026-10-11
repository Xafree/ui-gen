import type { ComponentModel } from '../../../../src/modules/sync/application/component-model.js';
import type { OpenedComponent } from '../../../../src/modules/sync/application/components/component-views.js';

const ANSWERING = `
addEventListener('message', (event) => {
  const port = event.ports[0];
  port.onmessage = (message) => {
    if (message.data.type === 'hit-test') port.postMessage({ type: 'hit', id: '0' });
    if (message.data.type === 'dark-preview') {
      port.postMessage({ type: 'rects', rects: [{ id: '0', x: message.data.enabled ? 7 : 3, y: 0, width: 1, height: 1 }] });
    }
  };
  port.postMessage({ type: 'oops' });
  port.postMessage({ type: 'rects', rects: [{ id: 'unknown', x: 0, y: 0, width: 1, height: 1 }] });
  port.postMessage({ type: 'ready' });
  port.postMessage({ type: 'rects', rects: [{ id: '0', x: 1, y: 2, width: 3, height: 4 }] });
});
`;

const SILENT = `addEventListener('message', () => undefined);`;

function documentUrl(script: string): string {
  return `data:text/html;charset=utf-8,${encodeURIComponent(`<script>${script}</script>`)}`;
}

export const ANSWERING_HARNESS_URL = documentUrl(ANSWERING);
export const SILENT_HARNESS_URL = documentUrl(SILENT);

export function modelWithVersion(version: string): ComponentModel {
  return {
    tag: 'hz-card',
    className: 'HzCard',
    version,
    template: [
      {
        kind: 'element',
        id: '0',
        tag: 'article',
        range: { start: 0, end: 1 },
        attributes: [],
        classes: [],
        children: [],
      },
    ],
    properties: [],
    variantProperty: null,
    variants: [],
    styles: [],
    diagnostics: [],
  };
}

export function openedComponent(version: string, path = 'E:/kit/src/hz-card.ts'): OpenedComponent {
  return { path, text: '', model: modelWithVersion(version), highlight: [] };
}
