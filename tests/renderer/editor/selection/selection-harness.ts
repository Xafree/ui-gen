const SELECTION_PAGE = `
addEventListener('message', (event) => {
  const port = event.ports[0];
  port.onmessage = (message) => {
    if (message.data.type === 'hit-test') port.postMessage({ type: 'hit', id: 'accent:0.1' });
  };
  port.postMessage({ type: 'ready' });
  port.postMessage({
    type: 'rects',
    rects: [
      { id: 'neutral:0', x: 10, y: 10, width: 200, height: 120 },
      { id: 'neutral:0.0', x: 30, y: 40, width: 100, height: 22 },
      { id: 'neutral:0.1', x: 30, y: 80, width: 80, height: 14 },
      { id: 'accent:0', x: 300, y: 10, width: 200, height: 120 },
      { id: 'accent:0.0', x: 320, y: 40, width: 100, height: 22 },
      { id: 'accent:0.1', x: 320, y: 80, width: 80, height: 14 },
    ],
  });
});
`;

export const SELECTION_HARNESS_URL = `data:text/html;charset=utf-8,${encodeURIComponent(`<script>${SELECTION_PAGE}</script>`)}`;
