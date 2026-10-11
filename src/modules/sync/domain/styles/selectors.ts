import type { StyleState, StyleVariant } from '../model/component-model.js';
import type { ElementNode } from '../model/template-node.js';
import { splitTopLevel } from '../rules/top-level-split.js';

type Combinator = 'descendant' | 'child' | 'sibling';

interface AttributeCondition {
  readonly name: string;
  readonly value: string | null;
}

interface Compound {
  tag: string | null;
  readonly classes: string[];
  readonly attributes: AttributeCondition[];
  host: boolean;
  slotted: boolean;
}

interface Link {
  readonly combinator: Combinator;
  readonly compound: Compound;
}

export type SelectorList = readonly (readonly Link[])[];

const TOKEN = /([>+~])|\s+|(:{1,2}[\w-]+(?:\([^)]*\))?|[.#][\w-]+|\[[^\]]*\]|[\w*-]+)/g;
const ATTRIBUTE = /^\[\s*([\w-]+)\s*(?:=\s*(?:"([^"]*)"|'([^']*)'|([^\s\]]+)))?/;
const PSEUDO_NAME = /^::?([\w-]+)/;
const STATE_MARKER = /:(hover|focus-visible|focus|active|disabled)(?![\w-])|\[disabled\]/g;
const NOT_GROUP = /:not\([^)]*\)/g;
const HOST_ATTRIBUTE = /:host\(\s*\[\s*([\w-]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([\w-]+))\s*\]\s*\)/;
const BOUND_ATTRIBUTE_PREFIX = /^[?.]/;

const STATES: ReadonlyMap<string, StyleState> = new Map([
  ['hover', 'hover'],
  ['focus-visible', 'focus'],
  ['focus', 'focus'],
  ['active', 'active'],
  ['disabled', 'disabled'],
]);

function newCompound(): Compound {
  return { tag: null, classes: [], attributes: [], host: false, slotted: false };
}

function attributeCondition(part: string): AttributeCondition | null {
  const match = ATTRIBUTE.exec(part);
  const name = match?.[1];
  return name === undefined ? null : { name: name.toLowerCase(), value: match?.[2] ?? match?.[3] ?? match?.[4] ?? null };
}

function addPart(compound: Compound, part: string): void {
  const head = part.charAt(0);
  if (head === '.') {
    compound.classes.push(part.slice(1));
  } else if (head === '#') {
    compound.attributes.push({ name: 'id', value: part.slice(1) });
  } else if (head === '[') {
    const condition = attributeCondition(part);
    if (condition !== null) {
      compound.attributes.push(condition);
    }
  } else if (head === ':') {
    const pseudo = PSEUDO_NAME.exec(part)?.[1] ?? '';
    compound.host ||= pseudo.startsWith('host');
    compound.slotted ||= pseudo === 'slotted';
  } else if (part !== '*') {
    compound.tag = part.toLowerCase();
  }
}

function parseComplexSelector(selector: string): Link[] {
  const links: Link[] = [];
  let combinator: Combinator = 'descendant';
  let compound: Compound | null = null;
  for (const [, symbol, part] of selector.matchAll(TOKEN)) {
    if (part === undefined) {
      if (symbol !== undefined) {
        combinator = symbol === '>' ? 'child' : 'sibling';
      } else if (compound !== null) {
        combinator = 'descendant';
      }
      compound = null;
    } else {
      if (compound === null) {
        compound = newCompound();
        links.push({ combinator, compound });
      }
      addPart(compound, part);
    }
  }
  return links;
}

export function parseSelectors(selector: string): SelectorList {
  return splitTopLevel(selector, (character) => character === ',').map(({ text }) => parseComplexSelector(text));
}

function hasAttribute(element: ElementNode, { name, value }: AttributeCondition): boolean {
  return element.attributes.some(
    (attribute) =>
      attribute.name.replace(BOUND_ATTRIBUTE_PREFIX, '').toLowerCase() === name &&
      (value === null || attribute.value === value),
  );
}

function matchesCompound(compound: Compound, element: ElementNode): boolean {
  return (
    !compound.host &&
    !compound.slotted &&
    (compound.tag === null || compound.tag === element.tag) &&
    compound.classes.every((name) => element.classes.includes(name)) &&
    compound.attributes.every((condition) => hasAttribute(element, condition))
  );
}

function matchesBefore(chain: readonly Link[], combinator: Combinator, ancestors: readonly ElementNode[]): boolean {
  const left = chain.at(-1);
  if (left === undefined || combinator === 'sibling') {
    return true;
  }
  if (left.compound.host) {
    return combinator === 'descendant' || ancestors.length === 0;
  }
  if (combinator === 'child') {
    const parent = ancestors.at(-1);
    return parent !== undefined && matchesChain(chain, parent, ancestors.slice(0, -1));
  }
  return ancestors.some((ancestor, index) => matchesChain(chain, ancestor, ancestors.slice(0, index)));
}

function matchesChain(chain: readonly Link[], element: ElementNode, ancestors: readonly ElementNode[]): boolean {
  const last = chain.at(-1);
  return (
    last !== undefined &&
    matchesCompound(last.compound, element) &&
    matchesBefore(chain.slice(0, -1), last.combinator, ancestors)
  );
}

export function matchesSelectors(
  selectors: SelectorList,
  element: ElementNode,
  ancestors: readonly ElementNode[],
): boolean {
  return selectors.some((chain) => matchesChain(chain, element, ancestors));
}

export function stateOf(selector: string): StyleState | null {
  const last = [...selector.replaceAll(NOT_GROUP, '').matchAll(STATE_MARKER)].at(-1);
  return last === undefined ? null : (STATES.get(last[1] ?? 'disabled') ?? null);
}

export function variantOf(selector: string): StyleVariant | null {
  const match = HOST_ATTRIBUTE.exec(selector);
  const property = match?.[1];
  const value = match?.[2] ?? match?.[3] ?? match?.[4];
  return property === undefined || value === undefined ? null : { property, value };
}
