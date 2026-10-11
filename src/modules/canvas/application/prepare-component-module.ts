import type { CanvasCompute } from '../ports/canvas-compute.js';
import { instrumentComponentSource } from './instrument-component.js';

export async function prepareComponentModule(
  compute: CanvasCompute,
  source: string,
  tsconfigText: string | null,
  fileName: string,
): Promise<string> {
  const model = await compute.readModel(source);
  return compute.transpile(instrumentComponentSource(source, model), tsconfigText, fileName);
}
