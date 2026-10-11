import type { ComponentModel } from '../domain/model/component-model.js';
import type { HighlightToken } from './code-highlighter.js';

export type AnalyzerStatus = 'ready' | 'restarting';

export interface ComponentAnalysis {
  readonly model: ComponentModel;
  readonly highlight: readonly HighlightToken[];
}

export interface ComponentAnalyzer {
  readModel(text: string): Promise<ComponentModel>;
  analyze(text: string): Promise<ComponentAnalysis>;
}
