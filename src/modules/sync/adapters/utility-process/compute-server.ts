import type { ComponentModel } from '../../domain/model/component-model.js';
import type { CodeHighlighter } from '../../ports/code-highlighter.js';
import type { AnalysisRequest, ComputeReply, ComputeRequest, TranspileRequest } from './compute-protocol.js';

export interface ComputeEngine {
  readonly readModel: (text: string) => ComponentModel;
  readonly highlighter: CodeHighlighter;
}

export type ModuleTranspile = (source: string, tsconfigText: string | null, fileName: string) => string;

export function createComputeServer({ readModel, highlighter }: ComputeEngine) {
  return ({ id, operation, text }: AnalysisRequest): ComputeReply => {
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

export function createTranspileServer(transpile: ModuleTranspile) {
  return ({ id, text, tsconfig, fileName }: TranspileRequest): ComputeReply => {
    try {
      return { outcome: 'transpiled', id, code: transpile(text, tsconfig, fileName) };
    } catch (cause) {
      console.error('The transpile request failed', cause);
      return { outcome: 'failed', id };
    }
  };
}

export function routeComputeRequests(
  analyze: (request: AnalysisRequest) => ComputeReply,
  transpile: (request: TranspileRequest) => ComputeReply,
) {
  return (request: ComputeRequest): ComputeReply =>
    request.operation === 'transpile' ? transpile(request) : analyze(request);
}
