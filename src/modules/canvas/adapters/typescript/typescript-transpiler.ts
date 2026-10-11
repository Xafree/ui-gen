import ts from 'typescript';

const DEFAULT_COMPILER_OPTIONS: Readonly<Record<string, unknown>> = {
  target: 'ES2022',
  experimentalDecorators: true,
  useDefineForClassFields: false,
};

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function declaredOptions(tsconfigText: string | null): Readonly<Record<string, unknown>> {
  if (tsconfigText === null) return DEFAULT_COMPILER_OPTIONS;
  const parsed = ts.parseConfigFileTextToJson('tsconfig.json', tsconfigText);
  const config: unknown = parsed.config;
  if (parsed.error !== undefined || !isRecord(config)) throw new Error('The tsconfig.json of the project cannot be read');
  const compilerOptions = config['compilerOptions'];
  return isRecord(compilerOptions) ? compilerOptions : {};
}

function compilerOptionsOf(tsconfigText: string | null): ts.CompilerOptions {
  const { options, errors } = ts.convertCompilerOptionsFromJson(declaredOptions(tsconfigText), '.');
  if (errors.length > 0) throw new Error('The compiler options of the project cannot be read');
  const { target } = options;
  return {
    ...options,
    target: target !== undefined && target >= ts.ScriptTarget.ES2020 ? target : ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
    sourceMap: false,
    inlineSourceMap: false,
    declaration: false,
  };
}

export function transpileProjectModule(source: string, tsconfigText: string | null, fileName: string): string {
  const { outputText, diagnostics } = ts.transpileModule(source, {
    compilerOptions: compilerOptionsOf(tsconfigText),
    fileName,
    reportDiagnostics: true,
  });
  if ((diagnostics ?? []).some(({ category }) => category === ts.DiagnosticCategory.Error)) {
    throw new Error('The module has syntax errors');
  }
  return outputText;
}
