import type { OpenProjectOutcome, ProjectDependencies } from '../../../../../src/modules/project/application/open/project-outcomes';
import type { RecentProject } from '../../../../../src/modules/project/domain/recents/recent-projects';
import { ProjectWriteError } from '../../../../../src/modules/project/ports/project-file-system';
import { InMemoryProjectFileSystem } from '../doubles/in-memory-project-file-system';
import { InMemoryRecentProjectsStore } from '../doubles/in-memory-recent-projects-store';

export const LIBRARY_PATH = 'E:\\clients\\atlas-design-system';
export const NOW = new Date('2026-10-11T09:30:00.000Z');

export const ATLAS_PACKAGE_JSON = `{
  "name": "@atlas-design/components",
  "version": "2.4.1",
  "description": "Atlas design system web components",
  "type": "module",
  "main": "dist/index.js",
  "scripts": {
    "build": "tsc -p tsconfig.build.json",
    "test": "web-test-runner"
  },
  "dependencies": {
    "lit": "^3.2.0",
    "@lit/context": "^1.1.3"
  },
  "devDependencies": {
    "typescript": "~5.6.3"
  }
}
`;

export const ATLAS_SOURCES: readonly (readonly [string, string])[] = [
  [
    'src\\lib\\icon\\ph-icon.ts',
    `import { LitElement, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

// Wrapper around the Phosphor icon set, kept under its upstream tag.
@customElement('ph-icon')
export class PhIcon extends LitElement {
  @property() name = 'circle';

  render() {
    return html\`<svg part="svg"><use href="#\${this.name}"></use></svg>\`;
  }
}
`,
  ],
  [
    'src\\lib\\button\\at-button.ts',
    `import { LitElement, css, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/**
 * Primary action button.
 */
@customElement('at-button')
export class AtButton extends LitElement {
  static styles = css\`
    :host { display: inline-flex; }
    button { border-radius: var(--at-radius, 6px); }
  \`;

  @property({ reflect: true }) variant: 'primary' | 'ghost' = 'primary';

  render() {
    return html\`<button class=\${this.variant}><slot></slot></button>\`;
  }
}
`,
  ],
  [
    'src\\lib\\card\\at-card.ts',
    `import { LitElement, html } from 'lit';

export class AtCard extends LitElement {
  render() {
    return html\`<article><slot name="header"></slot><slot></slot></article>\`;
  }
}

customElements.define('at-card', AtCard);
`,
  ],
  [
    'src\\lib\\dialog\\at-dialog.ts',
    `import { LitElement, html } from 'lit';
import { customElement, state } from 'lit/decorators.js';

@customElement("at-dialog")
export class AtDialog extends LitElement {
  @state() private open = false;

  render() {
    return this.open ? html\`<dialog open><slot></slot></dialog>\` : html\`\`;
  }
}
`,
  ],
  [
    'src\\index.ts',
    `export * from './lib/button/at-button.js';
export * from './lib/card/at-card.js';
export * from './lib/dialog/at-dialog.js';
export * from './lib/icon/ph-icon.js';
`,
  ],
];

export const VALID_SETTINGS = `{
  "formatVersion": 1,
  "tagPrefix": "at-",
  "componentsDir": "src/lib"
}
`;

export const ATLAS_RECENT: RecentProject = {
  path: LIBRARY_PATH,
  name: 'Atlas maison',
  openedAt: '2026-10-09T16:05:12.000Z',
};

export const NOVA_RECENT: RecentProject = {
  path: 'C:\\Users\\Camille\\Documents\\nova-ui',
  name: 'Nova UI',
  openedAt: '2026-09-28T07:45:00.000Z',
};

export class FailingWriteFileSystem extends InMemoryProjectFileSystem {
  override writeFileAtomically(folder: string, relativePath: string): Promise<void> {
    return Promise.reject(new ProjectWriteError(this.childPath(folder, relativePath), 'EPERM'));
  }
}

export interface Harness {
  readonly fileSystem: InMemoryProjectFileSystem;
  readonly recents: InMemoryRecentProjectsStore;
  readonly deps: ProjectDependencies;
}

export function harness(
  fileSystem: InMemoryProjectFileSystem = new InMemoryProjectFileSystem(),
  recentProjects: readonly RecentProject[] = [],
): Harness {
  const recents = new InMemoryRecentProjectsStore(recentProjects);
  return { fileSystem, recents, deps: { fileSystem, recentProjects: recents, clock: () => new Date(NOW) } };
}

export function inLibrary(relativePath: string): string {
  return `${LIBRARY_PATH}\\${relativePath}`;
}

export function withAtlasLibrary(fileSystem: InMemoryProjectFileSystem = new InMemoryProjectFileSystem()): InMemoryProjectFileSystem {
  fileSystem.addFolder(LIBRARY_PATH).addFile(inLibrary('package.json'), ATLAS_PACKAGE_JSON);
  for (const [relativePath, content] of ATLAS_SOURCES) fileSystem.addFile(inLibrary(relativePath), content);
  return fileSystem;
}

export function expectOutcome<K extends OpenProjectOutcome['kind']>(
  outcome: OpenProjectOutcome,
  kind: K,
): Extract<OpenProjectOutcome, { kind: K }> {
  if (outcome.kind !== kind) throw new Error(`expected ${kind}, got ${JSON.stringify(outcome)}`);
  return outcome as Extract<OpenProjectOutcome, { kind: K }>;
}
