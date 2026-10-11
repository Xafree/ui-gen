type Scope = Document | ShadowRoot;

export interface ShownToast {
  readonly tone: string | null;
  readonly title: string;
  readonly message: string;
}

function scopesOf(root: Scope): Scope[] {
  const scopes: Scope[] = [root];
  for (const element of root.querySelectorAll('*')) {
    if (element.shadowRoot !== null) scopes.push(...scopesOf(element.shadowRoot));
  }
  return scopes;
}

export function deepQueryAll(selector: string, root: Scope = document): Element[] {
  return scopesOf(root).flatMap((scope) => [...scope.querySelectorAll(selector)]);
}

export function deepActiveElement(): Element | null {
  let active = document.activeElement;
  while (active?.shadowRoot?.activeElement) {
    active = active.shadowRoot.activeElement;
  }
  return active;
}

function textOf(element: Element | null): string {
  return (element?.textContent ?? '').replace(/\s+/g, ' ').trim();
}

export function shownToasts(): ShownToast[] {
  return deepQueryAll('.ui-toast').map((toast) => ({
    tone: /\bui-toast--([a-z]+)\b/.exec(toast.className)?.[1] ?? null,
    title: textOf(toast.querySelector('.ui-toast__title')),
    message: textOf(toast.querySelector('.ui-toast__text')),
  }));
}

export function removeShownToasts(): void {
  for (const toast of deepQueryAll('.ui-toast')) {
    toast.remove();
  }
}

export function visibleText(element: Element | null): string {
  return textOf(element);
}
