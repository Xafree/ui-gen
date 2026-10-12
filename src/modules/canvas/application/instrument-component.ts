import type {
  InstrumentableElement,
  InstrumentableModel,
  InstrumentableNode,
} from '../domain/model/instrumentable-model.js';
import { NODE_MARK_ATTRIBUTE } from '../domain/node-mark.js';

interface NodeMark {
  readonly offset: number;
  readonly id: string;
}

const START_TAG_OPENING = /^<[^\t\n\f\r />]+/;

function markOfElement(text: string, element: InstrumentableElement): NodeMark[] {
  const opening = START_TAG_OPENING.exec(text.slice(element.range.start, element.range.end));
  return opening === null ? [] : [{ offset: element.range.start + opening[0].length, id: element.id }];
}

function marksOf(text: string, nodes: readonly InstrumentableNode[]): NodeMark[] {
  return nodes.flatMap((node) => {
    switch (node.kind) {
      case 'element':
        return [...markOfElement(text, node), ...marksOf(text, node.children)];
      case 'conditional':
        return marksOf(text, [...node.whenTrue, ...node.whenFalse]);
      case 'text':
      case 'binding':
      case 'opaque':
        return [];
    }
  });
}

export function instrumentComponentSource(text: string, model: InstrumentableModel): string {
  const marks = marksOf(text, model.template).toSorted((first, second) => first.offset - second.offset);
  const pieces: string[] = [];
  let cursor = 0;
  for (const { offset, id } of marks) {
    pieces.push(text.slice(cursor, offset), ` ${NODE_MARK_ATTRIBUTE}="${id}"`);
    cursor = offset;
  }
  pieces.push(text.slice(cursor));
  return pieces.join('');
}
