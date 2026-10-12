import { samePath } from '../../domain/library/component-summary.js';
import type { ComponentModel } from '../../domain/model/component-model.js';
import type { ComponentAnalyzer } from '../../ports/component-analyzer.js';
import type { ComponentLibrary } from '../../ports/component-library.js';
import type { ComponentChange } from './component-views.js';

export const CHANGE_DEBOUNCE_MS = 50;

export interface ChangeFeedDependencies {
  readonly library: ComponentLibrary;
  readonly analyzer: ComponentAnalyzer;
  readonly publish: (change: ComponentChange) => void;
}

interface FollowedComponent {
  readonly path: string;
  text: string;
  lastReadable: ComponentModel | null;
}

function isReadable(model: ComponentModel): boolean {
  return model.diagnostics.every(({ severity }) => severity !== 'error');
}

export class ComponentChangeFeed {
  readonly #deps: ChangeFeedDependencies;
  readonly #stopWatching: () => void;
  #followed: FollowedComponent | null = null;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #generation = 0;
  #watching = true;

  constructor(deps: ChangeFeedDependencies, componentsDir: string) {
    this.#deps = deps;
    this.#stopWatching = deps.library.watch(
      componentsDir,
      (path) => {
        this.#schedule(path);
      },
      () => {
        this.#watching = false;
      },
    );
  }

  get isWatching(): boolean {
    return this.#watching;
  }

  follow(path: string, text: string, model: ComponentModel): void {
    this.#cancelPending();
    this.#followed = { path, text, lastReadable: isReadable(model) ? model : null };
    void this.#refresh(this.#generation);
  }

  stop(): void {
    this.#watching = false;
    this.#stopWatching();
    this.#cancelPending();
    this.#followed = null;
  }

  #cancelPending(): void {
    this.#generation += 1;
    if (this.#timer !== null) clearTimeout(this.#timer);
    this.#timer = null;
  }

  #schedule(changedPath: string): void {
    if (this.#followed === null || !samePath(this.#followed.path, changedPath)) return;
    if (this.#timer !== null) clearTimeout(this.#timer);
    const generation = ++this.#generation;
    this.#timer = setTimeout(() => {
      this.#timer = null;
      void this.#refresh(generation);
    }, CHANGE_DEBOUNCE_MS);
  }

  async #refresh(generation: number): Promise<void> {
    const followed = this.#followed;
    if (followed === null) return;
    try {
      const text = await this.#deps.library.readText(followed.path);
      if (text === followed.text) return;
      const { model, highlight } = await this.#deps.analyzer.analyze(text);
      if (generation !== this.#generation || this.#followed !== followed) return;
      followed.text = text;
      if (isReadable(model)) followed.lastReadable = model;
      this.#deps.publish({
        path: followed.path,
        text,
        model: followed.lastReadable ?? model,
        highlight,
        diagnostics: model.diagnostics,
      });
    } catch (cause) {
      console.warn('A modification of the open component could not be read', cause);
    }
  }
}
