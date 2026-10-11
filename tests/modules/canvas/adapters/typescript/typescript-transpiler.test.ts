import { describe, expect, it } from 'vitest';
import { transpileProjectModule } from '../../../../../src/modules/canvas/adapters/typescript/typescript-transpiler';

const DECORATED = `import { LitElement, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

@customElement('hz-badge')
export class HzBadge extends LitElement {
  @property() label: string = 'Pro';
  render() {
    return html\`<span>\${this.label}</span>\`;
  }
}
`;

const LEGACY_DECORATORS =
  '{ "compilerOptions": { "target": "ES2022", "experimentalDecorators": true, "useDefineForClassFields": false } }';

describe('transpiling a project module', () => {
  it('keeps bare imports and compiles decorators with the options of the project', () => {
    const code = transpileProjectModule(DECORATED, LEGACY_DECORATORS, 'hz-badge.ts');

    expect(code).toContain("from 'lit/decorators.js'");
    expect(code).toContain('__decorate');
    expect(code).not.toContain(': string');
  });

  it('reads a tsconfig written with comments and trailing commas', () => {
    const tsconfig = `{
      // the project options
      "compilerOptions": { "experimentalDecorators": true, },
    }`;

    expect(transpileProjectModule(DECORATED, tsconfig, 'hz-badge.ts')).toContain('__decorate');
  });

  it('decorates like Lit expects when the project has no tsconfig', () => {
    expect(transpileProjectModule(DECORATED, null, 'hz-badge.ts')).toContain('__decorate');
  });

  it('keeps standard decorators when the project does not ask for the legacy ones', () => {
    const code = transpileProjectModule(DECORATED, '{ "compilerOptions": { "target": "ES2022" } }', 'hz-badge.ts');

    expect(code).not.toContain('__decorate(');
    expect(code).toContain('__esDecorate');
  });

  it('emits an ES module with a modern target whatever the project asks for', () => {
    const tsconfig = '{ "compilerOptions": { "target": "ES5", "module": "commonjs" } }';

    const code = transpileProjectModule('export const answer = 6 * 7;\nexport class A { #x = 1; }\n', tsconfig, 'a.ts');

    expect(code).toContain('export const answer');
    expect(code).toContain('#x');
    expect(code).not.toContain('require(');
  });

  it('does not emit source maps', () => {
    const tsconfig = '{ "compilerOptions": { "sourceMap": true, "inlineSourceMap": true } }';

    expect(transpileProjectModule('export const a = 1;\n', tsconfig, 'a.ts')).not.toContain('sourceMappingURL');
  });

  it.each([
    ['a tsconfig that is not JSON', '{ "compilerOptions": '],
    ['a tsconfig that is not an object', '[]'],
  ])('refuses %s', (_label, tsconfig) => {
    expect(() => transpileProjectModule('export {}', tsconfig, 'a.ts')).toThrow(
      'The tsconfig.json of the project cannot be read',
    );
  });

  it('refuses compiler options it cannot convert', () => {
    expect(() => transpileProjectModule('export {}', '{ "compilerOptions": { "target": "ES1999" } }', 'a.ts')).toThrow(
      'The compiler options of the project cannot be read',
    );
  });

  it('tolerates a tsconfig without compiler options', () => {
    expect(transpileProjectModule('export const a = 1;\n', '{}', 'a.ts')).toContain('export const a');
  });

  it('refuses a module with syntax errors', () => {
    expect(() => transpileProjectModule('export const = ;', null, 'a.ts')).toThrow('The module has syntax errors');
  });
});
