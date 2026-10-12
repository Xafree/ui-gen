export interface InstrumentableElement {
  readonly kind: 'element';
  readonly id: string;
  readonly range: { readonly start: number; readonly end: number };
  readonly children: readonly InstrumentableNode[];
}

export interface InstrumentableConditional {
  readonly kind: 'conditional';
  readonly whenTrue: readonly InstrumentableNode[];
  readonly whenFalse: readonly InstrumentableNode[];
}

export interface InstrumentableLeaf {
  readonly kind: 'text' | 'binding' | 'opaque';
}

export type InstrumentableNode = InstrumentableElement | InstrumentableConditional | InstrumentableLeaf;

export interface InstrumentableModel {
  readonly template: readonly InstrumentableNode[];
}
