import type { Diagnostic } from '../../domain/model/diagnostic.js';
import type { ComponentModel } from '../../domain/model/component-model.js';
import type { HighlightToken } from '../../ports/code-highlighter.js';
import type { ComponentAnalyzer } from '../../ports/component-analyzer.js';
import type { ComponentLibrary } from '../../ports/component-library.js';
import type { LastOpenedStore } from '../../ports/last-opened-store.js';

export interface OpenedComponent {
  readonly path: string;
  readonly text: string;
  readonly model: ComponentModel;
  readonly highlight: readonly HighlightToken[];
}

export interface ComponentChange extends OpenedComponent {
  readonly diagnostics: readonly Diagnostic[];
}

export interface LibraryLocation {
  readonly projectPath: string;
  readonly componentsDir: string;
  readonly tagPrefix: string;
}

export interface ComponentDependencies {
  readonly library: ComponentLibrary;
  readonly analyzer: ComponentAnalyzer;
  readonly lastOpened: LastOpenedStore;
}
