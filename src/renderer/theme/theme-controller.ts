import type { ReactiveController, ReactiveControllerHost } from 'lit';
import { currentTheme, THEME_CHANGE_EVENT, type Theme } from './theme.js';

export class ThemeController implements ReactiveController {
  readonly #host: ReactiveControllerHost;
  readonly #scheme = matchMedia('(prefers-color-scheme: dark)');
  #theme: Theme = currentTheme();

  constructor(host: ReactiveControllerHost) {
    this.#host = host;
    host.addController(this);
  }

  get theme(): Theme {
    return this.#theme;
  }

  hostConnected(): void {
    document.addEventListener(THEME_CHANGE_EVENT, this.#sync);
    this.#scheme.addEventListener('change', this.#sync);
    this.#sync();
  }

  hostDisconnected(): void {
    document.removeEventListener(THEME_CHANGE_EVENT, this.#sync);
    this.#scheme.removeEventListener('change', this.#sync);
  }

  readonly #sync = (): void => {
    this.#theme = currentTheme();
    this.#host.requestUpdate();
  };
}
