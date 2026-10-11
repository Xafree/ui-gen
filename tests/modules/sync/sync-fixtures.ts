import { readFileSync } from 'node:fs';
import { createTypeScriptComponentParser } from '../../../src/modules/sync/adapters/typescript/typescript-component-parser';
import { readComponent } from '../../../src/modules/sync/application/read-component';

export type Model = ReturnType<typeof readComponent>;
export type TemplateNodeShape = Model['template'][number];
export type Shape = string | readonly [string, readonly Shape[]];

export const PRICING_CARD = 'hz-pricing-card.ts';
export const PLAN_BADGE = 'hz-plan-badge.ts';
export const FEATURE_LIST = 'hz-feature-list.ts';
export const ALL_FIXTURES = [PRICING_CARD, PLAN_BADGE, FEATURE_LIST] as const;

export const NOT_REPRESENTABLE_MESSAGE =
  "Cette expression ne peut pas être affichée dans l'éditeur visuel. Elle est conservée dans le code.";

export function readFixture(name: string): string {
  return readFileSync(new URL(`../../fixtures/components/${name}`, import.meta.url), 'utf8');
}

export function read(text: string): Model {
  return readComponent(text, createTypeScriptComponentParser());
}

export function readFixtureModel(name: string): { readonly text: string; readonly model: Model } {
  const text = readFixture(name);
  return { text, model: read(text) };
}

export function replaceOnce(text: string, search: string, replacement: string): string {
  const index = text.indexOf(search);
  if (index < 0 || text.indexOf(search, index + 1) >= 0) {
    throw new Error(`expected exactly one occurrence of ${search}`);
  }
  return text.slice(0, index) + replacement + text.slice(index + search.length);
}

export function sliceOf(text: string, range: { readonly start: number; readonly end: number }): string {
  return text.slice(range.start, range.end);
}

export function positionOf(text: string, offset: number): { readonly line: number; readonly column: number } {
  const before = text.slice(0, offset);
  const lastBreak = before.lastIndexOf('\n');
  return { line: before.split('\n').length, column: offset - lastBreak };
}

export function isBlankText(node: TemplateNodeShape): boolean {
  return node.kind === 'text' && node.text.trim() === '';
}

export function outline(nodes: readonly TemplateNodeShape[]): Shape[] {
  return nodes.filter((node) => !isBlankText(node)).map(shapeOf);
}

function shapeOf(node: TemplateNodeShape): Shape {
  switch (node.kind) {
    case 'element':
      return [node.tag + node.classes.map((name) => `.${name}`).join(''), outline(node.children)];
    case 'text':
      return `text:${node.text.trim()}`;
    case 'binding':
      return `binding:${node.property}`;
    case 'opaque':
      return 'opaque';
  }
}

export function allNodes(nodes: readonly TemplateNodeShape[]): TemplateNodeShape[] {
  return nodes.flatMap((node) => (node.kind === 'element' ? [node, ...allNodes(node.children)] : [node]));
}

export function findElement(
  nodes: readonly TemplateNodeShape[],
  tag: string,
): Extract<TemplateNodeShape, { kind: 'element' }> {
  for (const node of allNodes(nodes)) {
    if (node.kind === 'element' && node.tag === tag) {
      return node;
    }
  }
  throw new Error(`no <${tag}> in the template`);
}

export function findProperty(model: Model, name: string): Model['properties'][number] {
  const found = model.properties.find((property) => property.name === name);
  if (found === undefined) {
    throw new Error(`no declared property ${name}`);
  }
  return found;
}
