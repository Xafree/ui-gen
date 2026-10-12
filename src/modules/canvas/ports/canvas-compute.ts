import type { InstrumentableModel } from '../domain/model/instrumentable-model.js';

export interface CanvasCompute {
  readModel(text: string): Promise<InstrumentableModel>;
  transpile(source: string, tsconfigText: string | null, fileName: string): Promise<string>;
}
