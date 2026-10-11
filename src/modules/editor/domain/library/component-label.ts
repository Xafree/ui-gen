export interface LabelledComponent {
  readonly name: string;
  readonly tag: string;
}

export function componentLabel(component: LabelledComponent): string {
  return component.name.replaceAll(/\s+/g, '');
}

export function matchesSearch(component: LabelledComponent, search: string): boolean {
  const query = search.trim().toLowerCase();
  return query === '' || componentLabel(component).toLowerCase().includes(query) || component.tag.includes(query);
}

export function filterComponents<T extends LabelledComponent>(components: readonly T[], search: string): T[] {
  return components.filter((component) => matchesSearch(component, search));
}
