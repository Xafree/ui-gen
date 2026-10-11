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

const COMBINATOR_SYMBOL = /[>+~]/y;
const SPACES = /\s+/y;
const PSEUDO_HEAD = /:{1,2}[\w-]+/y;
const SIMPLE_PART = /[.#][\w-]+|[\w*-]+/y;
const ATTRIBUTE_NAME = /^\[\s*([\w-]+)\s*/;
const ATTRIBUTE_VALUE = /^=\s*(?:"([^"]*)"|'([^']*)'|([^\s\]]+))/;
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

type SelectorToken =
  | { readonly kind: 'combinator'; readonly symbol: string }
  | { readonly kind: 'space' }
  | { readonly kind: 'part'; readonly text: string };

function newCompound(): Compound {
  return { tag: null, classes: [], attributes: [], host: false, slotted: false };
}

function attributeCondition(part: string): AttributeCondition | null {
  const head = ATTRIBUTE_NAME.exec(part);
  const name = head?.[1];
  if (head === null || name === undefined) {
    return null;
  }
  const value = ATTRIBUTE_VALUE.exec(part.slice(head[0].length));
  return { name: name.toLowerCase(), value: value?.[1] ?? value?.[2] ?? value?.[3] ?? null };
}

function matchAt(pattern: RegExp, text: string, index: number): string | null {
  pattern.lastIndex = index;
  return pattern.exec(text)?.[0] ?? null;
}

function pseudoEnd(selector: string, index: number): number | null {
  const head = matchAt(PSEUDO_HEAD, selector, index);
  if (head === null) {
    return null;
  }
  const afterName = index + head.length;
  const close = selector.charAt(afterName) === '(' ? selector.indexOf(')', afterName + 1) : -1;
  return close < 0 ? afterName : close + 1;
}

function attributeBlockEnd(selector: string, index: number): number | null {
  const close = selector.indexOf(']', index + 1);
  return close < 0 ? null : close + 1;
}

function partEnd(selector: string, index: number): number | null {
  const head = selector.charAt(index);
  if (head === ':') {
    return pseudoEnd(selector, index);
  }
  if (head === '[') {
    return attributeBlockEnd(selector, index);
  }
  const simple = matchAt(SIMPLE_PART, selector, index);
  return simple === null ? null : index + simple.length;
}

function tokenAt(selector: string, index: number): { token: SelectorToken; end: number } | null {
  const symbol = matchAt(COMBINATOR_SYMBOL, selector, index);
  if (symbol !== null) {
    return { token: { kind: 'combinator', symbol }, end: index + 1 };
  }
  const spaces = matchAt(SPACES, selector, index);
  if (spaces !== null) {
    return { token: { kind: 'space' }, end: index + spaces.length };
  }
  const end = partEnd(selector, index);
  return end === null ? null : { token: { kind: 'part', text: selector.slice(index, end) }, end };
}

function tokenize(selector: string): SelectorToken[] {
  const tokens: SelectorToken[] = [];
  let index = 0;
  while (index < selector.length) {
    const found = tokenAt(selector, index);
    if (found === null) {
      index += 1;
    } else {
      tokens.push(found.token);
      index = found.end;
    }
  }
  return tokens;
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
  for (const token of tokenize(selector)) {
    if (token.kind === 'part') {
      if (compound === null) {
        compound = newCompound();
        links.push({ combinator, compound });
      }
      addPart(compound, token.text);
    } else {
      if (token.kind === 'combinator') {
        combinator = token.symbol === '>' ? 'child' : 'sibling';
      } else if (compound !== null) {
        combinator = 'descendant';
      }
      compound = null;
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
