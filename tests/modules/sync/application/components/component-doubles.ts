import type { ComponentChange } from '../../../../../src/modules/sync/application/components/component-views';
import type { ComponentModel } from '../../../../../src/modules/sync/domain/model/component-model';
import type { ComponentAnalysis, ComponentAnalyzer } from '../../../../../src/modules/sync/ports/component-analyzer';
import type { ComponentLibrary, StopWatching } from '../../../../../src/modules/sync/ports/component-library';
import type { LastOpenedStore } from '../../../../../src/modules/sync/ports/last-opened-store';

export const BROKEN = 'BROKEN';

const TAG_LINE = /^tag=(\S+)/;

export function modelOf(text: string): ComponentModel {
  const tag = TAG_LINE.exec(text)?.[1] ?? '';
  const diagnostics = text.includes(BROKEN)
    ? [{ range: { start: 0, end: 1 }, line: 1, column: 1, severity: 'error' as const, code: 'ts(1005)', message: 'Erreur' }]
    : [];
  return {
    tag,
    className: '',
    version: text,
    template: [],
    properties: [],
    variantProperty: null,
    variants: [],
    styles: [],
    diagnostics,
  };
}

export class FakeAnalyzer implements ComponentAnalyzer {
  readonly analyzed: string[] = [];
  failWith: Error | null = null;
  pause: Promise<void> | null = null;

  readModel(text: string): Promise<ComponentModel> {
    if (this.failWith !== null) return Promise.reject(this.failWith);
    return Promise.resolve(modelOf(text));
  }

  async analyze(text: string): Promise<ComponentAnalysis> {
    this.analyzed.push(text);
    if (this.failWith !== null) throw this.failWith;
    if (this.pause !== null) await this.pause;
    return { model: modelOf(text), highlight: [{ start: 0, end: text.length, className: 'tk-kw' }] };
  }
}

export class FakeLibrary implements ComponentLibrary {
  readonly files = new Map<string, string>();
  readonly watched: string[] = [];
  stops = 0;
  #listener: ((path: string) => void) | null = null;

  add(path: string, text: string): this {
    this.files.set(path, text);
    return this;
  }

  listFiles(): Promise<readonly string[]> {
    return Promise.resolve([...this.files.keys()]);
  }

  readText(path: string): Promise<string> {
    const text = this.files.get(path);
    return text === undefined ? Promise.reject(new Error(`No file ${path}`)) : Promise.resolve(text);
  }

  watch(componentsDir: string, onChange: (path: string) => void): StopWatching {
    this.watched.push(componentsDir);
    this.#listener = onChange;
    return () => {
      this.stops += 1;
      this.#listener = null;
    };
  }

  emit(path: string): void {
    this.#listener?.(path);
  }

  get isWatching(): boolean {
    return this.#listener !== null;
  }
}

export class MemoryLastOpened implements LastOpenedStore {
  readonly saved = new Map<string, string>();

  load(projectPath: string): Promise<string | null> {
    return Promise.resolve(this.saved.get(projectPath) ?? null);
  }

  save(projectPath: string, componentPath: string): Promise<void> {
    this.saved.set(projectPath, componentPath);
    return Promise.resolve();
  }
}

export class ChangeRecorder {
  readonly changes: ComponentChange[] = [];

  readonly publish = (change: ComponentChange): void => {
    this.changes.push(change);
  };
}
