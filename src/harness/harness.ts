import type { HarnessMessage, NodeRect } from '../modules/canvas/domain/harness-message.js';

interface RenderRequest {
  readonly component: string;
  readonly tag: string;
  readonly version: string;
  readonly property: string | null;
  readonly variants: readonly string[];
  readonly dark: boolean;
}

const DARK_THEME_ATTRIBUTE = 'data-hz-theme';
const NODE_MARK_DATASET_KEY = 'nodeMark';

let port: MessagePort | null = null;
let instances: readonly HTMLElement[] = [];
let rectsScheduled = false;

function readRequest(): RenderRequest | null {
  const parameters = new URLSearchParams(location.search);
  const component = parameters.get('component');
  const tag = parameters.get('tag');
  const version = parameters.get('version');
  if (component === null || tag === null || version === null) return null;
  return {
    component,
    tag,
    version,
    property: parameters.get('property'),
    variants: parameters.getAll('variant'),
    dark: parameters.get('dark') === '1',
  };
}

function markAttribute(): string {
  return document.documentElement.dataset[NODE_MARK_DATASET_KEY] ?? '';
}

function post(message: HarnessMessage): void {
  port?.postMessage(message);
}

function applyDarkPreview(enabled: boolean): void {
  if (enabled) document.documentElement.setAttribute(DARK_THEME_ATTRIBUTE, 'dark');
  else document.documentElement.removeAttribute(DARK_THEME_ATTRIBUTE);
}

function moduleUrl({ component, version }: RenderRequest): string {
  const path = component.split('/').map(encodeURIComponent).join('/');
  const url = new URL(`/${path}`, location.href);
  url.searchParams.set('v', version);
  return url.href;
}

function createInstance({ tag, property }: RenderRequest, variant: string | null): HTMLElement {
  const element = document.createElement(tag);
  if (property !== null && variant !== null) {
    element.setAttribute(property.toLowerCase(), variant);
    Reflect.set(element, property, variant);
  }
  return element;
}

async function settled(element: HTMLElement): Promise<void> {
  const pending: unknown = Reflect.get(element, 'updateComplete');
  if (pending instanceof Promise) await pending;
}

function layOutCanvas(): void {
  Object.assign(document.body.style, {
    margin: '0',
    padding: '16px',
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'flex-start',
    gap: '16px',
  });
}

async function render(request: RenderRequest): Promise<void> {
  applyDarkPreview(request.dark);
  await import(moduleUrl(request));
  if (customElements.get(request.tag) === undefined) throw new Error('The component did not define its element');
  const variants = request.property === null || request.variants.length === 0 ? [null] : request.variants;
  instances = variants.map((variant) => createInstance(request, variant));
  layOutCanvas();
  document.body.replaceChildren(...instances);
  await Promise.all(instances.map(settled));
}

function markedElementsIn(root: Document | ShadowRoot): Element[] {
  const marked = [...root.querySelectorAll(`[${markAttribute()}]`)];
  const nested = [...root.querySelectorAll('*')].flatMap((element) =>
    element.shadowRoot === null ? [] : markedElementsIn(element.shadowRoot),
  );
  return [...marked, ...nested];
}

function measure(): NodeRect[] {
  return markedElementsIn(document).flatMap((element) => {
    const id = element.getAttribute(markAttribute());
    if (id === null) return [];
    const { x, y, width, height } = element.getBoundingClientRect();
    return [{ id, x, y, width, height }];
  });
}

function postRects(): void {
  post({ type: 'rects', rects: measure() });
}

function scheduleRects(): void {
  if (rectsScheduled) return;
  rectsScheduled = true;
  requestAnimationFrame(() => {
    rectsScheduled = false;
    postRects();
  });
}

function deepElementAt(x: number, y: number): Element | null {
  let found = document.elementFromPoint(x, y);
  while (found?.shadowRoot) {
    const inner: Element | null = found.shadowRoot.elementFromPoint(x, y);
    if (inner === null || inner === found) break;
    found = inner;
  }
  return found;
}

function markedAncestorId(start: Element | null): string | null {
  let current = start;
  while (current !== null) {
    const id = current.getAttribute(markAttribute());
    if (id !== null) return id;
    const root = current.getRootNode();
    current = current.parentElement ?? (root instanceof ShadowRoot ? root.host : null);
  }
  return null;
}

function readCoordinate(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function handleCommand(data: unknown): void {
  if (typeof data !== 'object' || data === null) return;
  const type: unknown = Reflect.get(data, 'type');
  if (type === 'hit-test') {
    const x = readCoordinate(Reflect.get(data, 'x'));
    const y = readCoordinate(Reflect.get(data, 'y'));
    if (x !== null && y !== null) post({ type: 'hit', id: markedAncestorId(deepElementAt(x, y)) });
  } else if (type === 'dark-preview') {
    const enabled: unknown = Reflect.get(data, 'enabled');
    if (typeof enabled === 'boolean') applyDarkPreview(enabled);
  }
}

function observeLayout(): void {
  const observer = new ResizeObserver(scheduleRects);
  observer.observe(document.body);
  for (const instance of instances) observer.observe(instance);
}

function connect(rendering: Promise<void>, received: MessagePort): void {
  port = received;
  port.onmessage = (event: MessageEvent<unknown>) => {
    handleCommand(event.data);
  };
  const announce = (): void => {
    post({ type: 'ready' });
    postRects();
    observeLayout();
  };
  rendering.then(announce, () => undefined);
}

function start(): void {
  const request = readRequest();
  if (request === null) {
    console.error('The canvas request is incomplete');
    return;
  }
  const rendering = render(request);
  rendering.catch((error: unknown) => {
    console.error('The component could not be rendered', error);
  });
  window.addEventListener('message', (event: MessageEvent<unknown>) => {
    const [received] = event.ports;
    if (port === null && event.source === window.parent && received !== undefined) connect(rendering, received);
  });
}

start();
