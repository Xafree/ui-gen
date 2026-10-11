import type { ComponentModel } from '../../domain/model/component-model.js';
import type { HighlightToken } from '../../ports/code-highlighter.js';

export const MAX_COMPUTE_TEXT_LENGTH = 1_000_000;

export type ComputeOperation = 'read-model' | 'analyze';

export interface ComputeRequest {
  readonly id: number;
  readonly operation: ComputeOperation;
  readonly text: string;
}

export interface ComputeReady {
  readonly outcome: 'ready';
}

export type ComputeReply =
  | { readonly outcome: 'model'; readonly id: number; readonly model: ComponentModel }
  | {
      readonly outcome: 'analysis';
      readonly id: number;
      readonly model: ComponentModel;
      readonly highlight: readonly HighlightToken[];
    }
  | { readonly outcome: 'failed'; readonly id: number };

export type ComputeMessage = ComputeReady | ComputeReply;

const MODEL_TEXT_KEYS = ['tag', 'className', 'version'] as const;
const MODEL_LIST_KEYS = ['template', 'properties', 'variants', 'styles', 'diagnostics'] as const;

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isComponentModel(value: unknown): value is ComponentModel {
  return (
    isRecord(value) &&
    MODEL_TEXT_KEYS.every((key) => typeof value[key] === 'string') &&
    MODEL_LIST_KEYS.every((key) => Array.isArray(value[key])) &&
    (value['variantProperty'] === null || typeof value['variantProperty'] === 'string')
  );
}

function isHighlight(value: unknown): value is readonly HighlightToken[] {
  return Array.isArray(value);
}

function isId(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

export function readComputeRequest(value: unknown): ComputeRequest | null {
  if (!isRecord(value)) return null;
  const { id, operation, text } = value;
  const isOperation = operation === 'read-model' || operation === 'analyze';
  const isText = typeof text === 'string' && text.length <= MAX_COMPUTE_TEXT_LENGTH;
  return isId(id) && isOperation && isText ? { id, operation, text } : null;
}

export function readComputeMessage(value: unknown): ComputeMessage | null {
  if (!isRecord(value)) return null;
  const { outcome, id, model, highlight } = value;
  if (outcome === 'ready') return { outcome };
  if (!isId(id)) return null;
  if (outcome === 'failed') return { outcome, id };
  if (!isComponentModel(model)) return null;
  if (outcome === 'model') return { outcome, id, model };
  return outcome === 'analysis' && isHighlight(highlight) ? { outcome, id, model, highlight } : null;
}
