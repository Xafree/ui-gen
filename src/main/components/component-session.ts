import { join } from 'node:path';
import { ComponentChangeFeed } from '../../modules/sync/application/components/component-changes.js';
import type {
  ComponentChange,
  ComponentDependencies,
  LibraryLocation,
  OpenedComponent,
} from '../../modules/sync/application/components/component-views.js';
import { findListedComponent, listComponents } from '../../modules/sync/application/components/list-components.js';
import { lastOpenedComponent, openComponent } from '../../modules/sync/application/components/open-component.js';
import type { ComponentSummary } from '../../modules/sync/domain/library/component-summary.js';
import type { ProjectSession } from '../session/project-session.js';

export interface ComponentSessionDependencies extends ComponentDependencies {
  readonly project: ProjectSession;
  readonly publish: (change: ComponentChange) => void;
}

interface WatchedLibrary {
  readonly location: LibraryLocation;
  readonly feed: ComponentChangeFeed;
}

export class ComponentSession {
  readonly #deps: ComponentSessionDependencies;
  #watched: WatchedLibrary | null = null;

  constructor(deps: ComponentSessionDependencies) {
    this.#deps = deps;
    deps.project.onClosed(() => {
      this.dispose();
    });
  }

  async list(): Promise<ComponentSummary[]> {
    const location = this.#location();
    return location === null ? [] : listComponents(this.#deps, location);
  }

  async resolve(path: string): Promise<string | null> {
    const location = this.#location();
    return location === null ? null : findListedComponent(this.#deps, location, path);
  }

  async open(path: string): Promise<OpenedComponent> {
    const location = this.#location();
    if (location === null) throw new Error('No project is open');
    const opened = await openComponent(this.#deps, location, path);
    this.#feedFor(location).follow(opened.path, opened.text, opened.model);
    return opened;
  }

  async lastOpened(): Promise<string | null> {
    const location = this.#location();
    return location === null ? null : lastOpenedComponent(this.#deps, location);
  }

  dispose(): void {
    this.#watched?.feed.stop();
    this.#watched = null;
  }

  #location(): LibraryLocation | null {
    const project = this.#deps.project.currentProject;
    if (project === null) return null;
    const { componentsDir, tagPrefix } = project.settings;
    return { projectPath: project.path, componentsDir: join(project.path, ...componentsDir.split('/')), tagPrefix };
  }

  #feedFor(location: LibraryLocation): ComponentChangeFeed {
    const watched = this.#watched;
    if (watched?.location.componentsDir === location.componentsDir && watched.feed.isWatching) return watched.feed;
    watched?.feed.stop();
    const feed = new ComponentChangeFeed(
      {
        library: this.#deps.library,
        analyzer: this.#deps.analyzer,
        publish: (change) => {
          if (this.#deps.project.currentProject?.path === location.projectPath) this.#deps.publish(change);
        },
      },
      location.componentsDir,
    );
    this.#watched = { location, feed };
    return feed;
  }
}
