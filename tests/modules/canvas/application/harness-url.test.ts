import { describe, expect, it } from 'vitest';
import {
  buildHarnessUrl,
  collectNodeIds,
  projectRelativePath,
} from '../../../../src/modules/canvas/application/harness-url';
import { PRICING_CARD, readFixtureModel } from '../../sync/sync-fixtures';

describe('project relative path', () => {
  it('relativizes a Windows path whatever its separators and case', () => {
    expect(projectRelativePath('E:\\projets\\Kit', 'e:\\projets\\kit\\src\\a.ts')).toBe('src/a.ts');
    expect(projectRelativePath('E:/projets/kit/', 'E:/projets/kit/src/a.ts')).toBe('src/a.ts');
  });

  it('refuses a file outside the project, a sibling folder and the project itself', () => {
    expect(projectRelativePath('E:/projets/kit', 'E:/projets/other/a.ts')).toBeNull();
    expect(projectRelativePath('E:/projets/kit', 'E:/projets/kit-two/a.ts')).toBeNull();
    expect(projectRelativePath('E:/projets/kit', 'E:/projets/kit/')).toBeNull();
  });
});

describe('harness url', () => {
  const { model } = readFixtureModel(PRICING_CARD);

  it('describes the component, its version and its variants', () => {
    const url = buildHarnessUrl({
      projectPath: 'E:/p',
      componentPath: 'E:/p/src/components/hz-pricing-card.ts',
      model,
      dark: false,
    });
    const parsed = new URL(url ?? '');

    expect(parsed.protocol).toBe('uigen-canvas:');
    expect(parsed.pathname).toBe('/__studio/harness.html');
    expect(parsed.searchParams.get('component')).toBe('src/components/hz-pricing-card.ts');
    expect(parsed.searchParams.get('tag')).toBe(model.tag);
    expect(parsed.searchParams.get('version')).toBe(model.version);
    expect(parsed.searchParams.get('dark')).toBe('0');
    expect(parsed.searchParams.get('property')).toBe(model.variantProperty);
    expect(model.variants.length).toBeGreaterThan(0);
    expect(parsed.searchParams.getAll('variant')).toEqual([...model.variants]);
  });

  it('carries the dark preview and omits the variants of a component without any', () => {
    const plain = { ...model, variantProperty: null, variants: [] };

    const parsed = new URL(
      buildHarnessUrl({ projectPath: 'E:/p', componentPath: 'E:/p/a.ts', model: plain, dark: true }) ?? '',
    );

    expect(parsed.searchParams.get('dark')).toBe('1');
    expect(parsed.searchParams.has('property')).toBe(false);
    expect(parsed.searchParams.has('variant')).toBe(false);
  });

  it('has no url for a component outside the project', () => {
    expect(buildHarnessUrl({ projectPath: 'E:/p', componentPath: 'E:/q/a.ts', model, dark: false })).toBeNull();
  });
});

describe('node ids of a model', () => {
  it('lists every node of the template', () => {
    const ids = collectNodeIds(readFixtureModel(PRICING_CARD).model);

    expect(ids.has('0')).toBe(true);
    expect(ids.size).toBeGreaterThan(5);
  });
});
