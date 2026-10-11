import type { ComponentModel } from '../../sync/application/component-model.js';

export interface CanvasCompute {
  readModel(text: string): Promise<ComponentModel>;
  transpile(source: string, tsconfigText: string | null, fileName: string): Promise<string>;
}
