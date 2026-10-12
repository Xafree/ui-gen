export function markedElementsIn(root: Element | ShadowRoot, mark: string, instanceTag: string): Element[] {
  const marked = [...root.querySelectorAll(`[${mark}]`)];
  const nested = [...root.querySelectorAll(instanceTag)].flatMap((host) =>
    host.shadowRoot === null ? [] : markedElementsIn(host.shadowRoot, mark, instanceTag),
  );
  return [...marked, ...nested];
}
