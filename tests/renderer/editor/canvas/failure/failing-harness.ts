import { connectingPrelude } from '../fake-harness.js';

function documentUrl(script: string): string {
  return `data:text/html;charset=utf-8,${encodeURIComponent(`<script>${script}</script>`)}`;
}

export const FAILING_HARNESS_URL = documentUrl(`${connectingPrelude()}
{
  connect();
  port.postMessage({ type: 'error' });
}
`);

export const TALL_HARNESS_URL = documentUrl(`${connectingPrelude()}
{
  connect();
  port.postMessage({ type: 'ready' });
  port.postMessage({ type: 'rects', rects: [{ id: 'starter:0', x: 0, y: 0, width: 10, height: 1e300 }] });
}
`);
