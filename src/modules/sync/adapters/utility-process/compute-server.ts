import type { ComponentModel } from '../../domain/model/component-model.js';
import type { CodeHighlighter } from '../../ports/code-highlighter.js';
import type { ComputeReply, ComputeRequest } from './compute-protocol.js';

export interface ComputeEngine {
  readonly readModel: (text: string) => ComponentModel;
  readonly highlighter: CodeHighlighter;
}

export function createComputeServer({ readModel, highlighter }: ComputeEngine) {
  return ({ id, operation, text }: ComputeRequest): ComputeReply => {
    try {
      const model = readModel(text);
      if (operation === 'read-model') return { outcome: 'model', id, model };
      return { outcome: 'analysis', id, model, highlight: highlighter.highlight(text) };
    } catch (cause) {
      console.error(`The ${operation} request failed`, cause);
      return { outcome: 'failed', id };
    }
  };
}
