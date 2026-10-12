import { afterEach, describe, expect, it } from 'vitest';
import { markedElementsIn } from '../../src/harness/marked-elements.js';

const MARK = 'data-test-node';
const hosts: HTMLElement[] = [];

function markedChild(id: string): HTMLElement {
  const element = document.createElement('div');
  element.setAttribute(MARK, id);
  return element;
}

function hostWith(tag: string, children: readonly Node[]): HTMLElement {
  const host = document.createElement(tag);
  host.attachShadow({ mode: 'open' }).append(...children);
  document.body.append(host);
  hosts.push(host);
  return host;
}

function rootOf(host: HTMLElement): ShadowRoot {
  if (host.shadowRoot === null) throw new Error('The host has no shadow root');
  return host.shadowRoot;
}

afterEach(() => {
  for (const host of hosts.splice(0)) host.remove();
});

describe('the marked elements of an instance', () => {
  it('lists the marked elements of the shadow root of the instance', () => {
    const instance = hostWith('x-card', [markedChild('0'), markedChild('0.1')]);

    const ids = markedElementsIn(rootOf(instance), MARK, 'x-card').map((element) => element.getAttribute(MARK));

    expect(ids).toEqual(['0', '0.1']);
  });

  it('does not enter the shadow root of a nested component of another tag', () => {
    const inner = hostWith('x-button', [markedChild('0')]);
    inner.remove();
    const instance = hostWith('x-card', [markedChild('0'), inner]);

    const ids = markedElementsIn(rootOf(instance), MARK, 'x-card').map((element) => element.getAttribute(MARK));

    expect(ids).toEqual(['0']);
  });

  it('lists the marked elements of a component without shadow root', () => {
    const light = document.createElement('x-plain');
    light.append(markedChild('0'));
    document.body.append(light);
    hosts.push(light);

    expect(markedElementsIn(light, MARK, 'x-plain')).toHaveLength(1);
  });
});
