import { describe, expect, it } from 'vitest';
import { prepareComponentModule } from '../../../../src/modules/canvas/application/prepare-component-module';
import type { CanvasCompute } from '../../../../src/modules/canvas/ports/canvas-compute';
import { PRICING_CARD, readFixtureModel } from '../../sync/sync-fixtures';

describe('preparing a component module', () => {
  it('transpiles the instrumented copy, never the original text', async () => {
    const { text, model } = readFixtureModel(PRICING_CARD);
    const seen: { source: string; tsconfig: string | null; fileName: string }[] = [];
    const compute: CanvasCompute = {
      readModel: () => Promise.resolve(model),
      transpile: (source, tsconfig, fileName) => {
        seen.push({ source, tsconfig, fileName });
        return Promise.resolve('compiled');
      },
    };
    const code = await prepareComponentModule(compute, text, '{"compilerOptions":{}}', 'hz-pricing-card.ts');

    expect(code).toBe('compiled');
    expect(seen).toHaveLength(1);
    expect(seen[0]?.source).toContain('data-uigen-node="0"');
    expect(seen[0]?.source).not.toBe(text);
    expect(seen[0]?.tsconfig).toBe('{"compilerOptions":{}}');
    expect(seen[0]?.fileName).toBe('hz-pricing-card.ts');
  });

  it('lets a failure of the compute process through', async () => {
    const compute: CanvasCompute = {
      readModel: () => Promise.reject(new Error('unavailable')),
      transpile: () => Promise.resolve(''),
    };

    await expect(prepareComponentModule(compute, 'x', null, 'a.ts')).rejects.toThrow('unavailable');
  });
});
