import type { ComponentModel, NodeId, TemplateNode } from '../../sync/application/component-model.js';
import { HARNESS_PAGE_URL } from '../domain/canvas-address.js';

export interface HarnessRequest {
  readonly projectPath: string;
  readonly componentPath: string;
  readonly model: ComponentModel;
  readonly dark: boolean;
}

function normalized(path: string): string {
  return path.replaceAll('\\', '/');
}

export function projectRelativePath(projectPath: string, componentPath: string): string | null {
  const root = normalized(projectPath).replace(/\/+$/, '');
  const file = normalized(componentPath);
  const prefix = `${root}/`;
  if (!file.toLowerCase().startsWith(prefix.toLowerCase())) return null;
  const relative = file.slice(prefix.length);
  return relative === '' ? null : relative;
}

export function buildHarnessUrl({ projectPath, componentPath, model, dark }: HarnessRequest): string | null {
  const component = projectRelativePath(projectPath, componentPath);
  if (component === null) return null;
  const parameters = new URLSearchParams({ component, tag: model.tag, version: model.version });
  if (model.variantProperty !== null) {
    parameters.set('property', model.variantProperty);
    for (const variant of model.variants) parameters.append('variant', variant);
  }
  parameters.set('dark', dark ? '1' : '0');
  return `${HARNESS_PAGE_URL}?${parameters.toString()}`;
}

function idsOf(nodes: readonly TemplateNode[]): NodeId[] {
  return nodes.flatMap((node) => {
    switch (node.kind) {
      case 'element':
        return [node.id, ...idsOf(node.children)];
      case 'conditional':
        return [node.id, ...idsOf(node.whenTrue), ...idsOf(node.whenFalse)];
      case 'text':
      case 'binding':
      case 'opaque':
        return [node.id];
    }
  });
}

export function collectNodeIds(model: ComponentModel): Set<NodeId> {
  return new Set(idsOf(model.template));
}
