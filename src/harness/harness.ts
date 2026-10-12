import { instanceNodeId } from '../modules/canvas/application/instance-node-id.js';
import { readHarnessCommand, type HarnessCommand } from '../modules/canvas/domain/harness-command.js';
import { markedElementsIn } from './marked-elements.js';
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
interface HiddenStyle {
  readonly value: string;
  readonly priority: string;
}

let instanceTag = '';
let instances: readonly HTMLElement[] = [];
let instanceVariants: readonly (string | null)[] = [];
let hiddenIds: ReadonlySet<string> = new Set();
let lockedIds: ReadonlySet<string> = new Set();
let rectsScheduled = false;
const hiddenElements = new Map<Element, HiddenStyle>();

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

function enableDarkPreview(): void {
  document.documentElement.setAttribute(DARK_THEME_ATTRIBUTE, 'dark');
}

function disableDarkPreview(): void {
  document.documentElement.removeAttribute(DARK_THEME_ATTRIBUTE);
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
  instanceTag = request.tag;
  if (request.dark) enableDarkPreview();
  await import(moduleUrl(request));
  if (customElements.get(request.tag) === undefined) throw new Error('The component did not define its element');
  const variants = request.property === null || request.variants.length === 0 ? [null] : request.variants;
  instanceVariants = variants;
  instances = variants.map((variant) => createInstance(request, variant));
  layOutCanvas();
  document.body.replaceChildren(...instances);
  await Promise.all(instances.map(settled));
}

function instanceRoot(instance: HTMLElement): Element | ShadowRoot {
  return instance.shadowRoot ?? instance;
}

function layerIdOf(element: Element, variant: string | null): string | null {
  const nodeId = element.getAttribute(markAttribute());
  return nodeId === null ? null : instanceNodeId(variant, nodeId);
}

function measure(): NodeRect[] {
  return instances.flatMap((instance, index) =>
    markedElementsIn(instanceRoot(instance), markAttribute(), instanceTag).flatMap((element) => {
      const id = layerIdOf(element, instanceVariants[index] ?? null);
      if (id === null) return [];
      const { x, y, width, height } = element.getBoundingClientRect();
      return [{ id, x, y, width, height }];
    }),
  );
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

function parentOf(element: Element): Element | null {
  const root = element.getRootNode();
  return element.parentElement ?? (root instanceof ShadowRoot ? root.host : null);
}

function instanceIndexOf(start: Element): number {
  const elements: readonly Element[] = instances;
  let current: Element | null = start;
  while (current !== null) {
    const index = elements.indexOf(current);
    if (index >= 0) return index;
    current = parentOf(current);
  }
  return -1;
}

function selectableLayerId(start: Element | null): string | null {
  let current = start;
  while (current !== null) {
    const index = instanceIndexOf(current);
    const id = index < 0 ? null : layerIdOf(current, instanceVariants[index] ?? null);
    if (id !== null && !lockedIds.has(id)) return id;
    current = parentOf(current);
  }
  return null;
}

function hide(element: Element): void {
  if (!(element instanceof HTMLElement) || hiddenElements.has(element)) return;
  hiddenElements.set(element, {
    value: element.style.getPropertyValue('visibility'),
    priority: element.style.getPropertyPriority('visibility'),
  });
  element.style.setProperty('visibility', 'hidden', 'important');
}

function reveal(element: Element, previous: HiddenStyle): void {
  if (!(element instanceof HTMLElement)) return;
  element.style.removeProperty('visibility');
  if (previous.value !== '') element.style.setProperty('visibility', previous.value, previous.priority);
}

function hiddenTargets(): Element[] {
  const branches = instances.filter((_instance, index) => hiddenIds.has(instanceVariants[index] ?? ''));
  const layers = instances.flatMap((instance, index) =>
    markedElementsIn(instanceRoot(instance), markAttribute(), instanceTag).filter((element) => {
      const id = layerIdOf(element, instanceVariants[index] ?? null);
      return id !== null && hiddenIds.has(id);
    }),
  );
  return [...branches, ...layers];
}

function applyHidden(): void {
  const targets = new Set(hiddenTargets());
  for (const [element, previous] of hiddenElements) {
    if (targets.has(element)) continue;
    reveal(element, previous);
    hiddenElements.delete(element);
  }
  for (const element of targets) hide(element);
}

function run(command: HarnessCommand): void {
  switch (command.type) {
    case 'hit-test':
      post({ type: 'hit', id: selectableLayerId(deepElementAt(command.x, command.y)) });
      break;
    case 'dark-preview':
      if (command.enabled) enableDarkPreview();
      else disableDarkPreview();
      break;
    case 'layer-state':
      hiddenIds = new Set(command.hidden);
      lockedIds = new Set(command.locked);
      applyHidden();
      scheduleRects();
      break;
  }
}

function handleCommand(data: unknown): void {
  const command = readHarnessCommand(data);
  if (command === null) console.warn('The studio sent an unreadable command');
  else run(command);
}

function observeLayout(): void {
  const observer = new ResizeObserver(scheduleRects);
  observer.observe(document.body);
  for (const instance of instances) observer.observe(instance);
}

function connect(rendering: Promise<void>): void {
  const studioOrigin = location.ancestorOrigins[0];
  if (studioOrigin === undefined) {
    console.error('The canvas has no studio to connect to');
    return;
  }
  const channel = new MessageChannel();
  port = channel.port1;
  port.onmessage = (event: MessageEvent<unknown>) => {
    handleCommand(event.data);
  };
  window.parent.postMessage({ type: 'connect' }, studioOrigin, [channel.port2]);
  const announce = (): void => {
    post({ type: 'ready' });
    postRects();
    observeLayout();
  };
  rendering.then(announce, () => {
    post({ type: 'error' });
  });
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
  connect(rendering);
}

start();
