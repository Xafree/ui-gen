import type { ComponentModel, NodeId, TemplateNode } from '../../sync/application/component-model.js';
import { HARNESS_PAGE_URL } from '../domain/canvas-address.js';
import { instanceNodeId, renderedVariants } from './instance-node-id.js';

export interface HarnessRequest {
  readonly projectPath: string;
  readonly componentPath: string;
  readonly model: ComponentModel;
  readonly dark: boolean;
}

function normalized(path: string): string {
  return path.replaceAll('\\', '/');
}

function withoutTrailingSlashes(path: string): string {
  let end = path.length;
  while (end > 0 && path.charAt(end - 1) === '/') end -= 1;
  return path.slice(0, end);
}

export function projectRelativePath(projectPath: string, componentPath: string): string | null {
  const root = withoutTrailingSlashes(normalized(projectPath));
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

export function collectRenderedIds(model: ComponentModel): Set<string> {
  const nodeIds = collectNodeIds(model);
  return new Set(renderedVariants(model).flatMap((variant) => [...nodeIds].map((nodeId) => instanceNodeId(variant, nodeId))));
}
