import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  readEmptyPayload,
  readListedComponent,
  registerComponentHandlers,
  type ComponentUseCases,
} from '../../../src/main/components/component-handlers';
import { IPC_CHANNELS } from '../../../src/main/ipc/channels';
import { IpcRejectedError } from '../../../src/main/ipc/payloads';
import { FOREIGN_URL, FakeIpcRegistry, TRUSTED_URL, rejectionOf } from '../ipc/handlers/handler-harness';

const LISTED = 'E:\\kit\\src\\components\\hz-card.ts';
const OPENED = {
  path: LISTED,
  text: 'tag=hz-card',
  model: {
    tag: 'hz-card',
    className: 'HzCard',
    version: 'v',
    template: [],
    properties: [],
    variantProperty: null,
    variants: [],
    styles: [],
    diagnostics: [],
  },
  highlight: [],
};

class RecordingComponents implements ComponentUseCases {
  readonly calls: string[] = [];
  failResolve = false;

  list() {
    this.calls.push('list');
    return Promise.resolve([{ path: LISTED, tag: 'hz-card', name: 'Card' }]);
  }

  resolve(path: string) {
    this.calls.push(`resolve ${path}`);
    if (this.failResolve) return Promise.reject(new Error(`cannot read ${path}`));
    return Promise.resolve(path.toLowerCase() === LISTED.toLowerCase() ? LISTED : null);
  }

  open(path: string) {
    this.calls.push(`open ${path}`);
    return Promise.resolve({ ...OPENED, path });
  }

  lastOpened() {
    this.calls.push('lastOpened');
    return Promise.resolve(LISTED);
  }
}

function setUp() {
  const registry = new FakeIpcRegistry();
  const components = new RecordingComponents();
  const checkedSenders: string[] = [];
  registerComponentHandlers(registry, {
    components,
    isTrustedSender: (senderUrl) => {
      checkedSenders.push(senderUrl);
      return senderUrl === TRUSTED_URL;
    },
  });
  return { registry, components, checkedSenders };
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  vi.spyOn(console, 'error').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('component channels', () => {
  it('registers the three request channels', () => {
    expect(setUp().registry.channels).toEqual(['component:list', 'component:open', 'component:last-opened']);
  });

  // covers: FCT-0026
  it('lists the components and answers the last opened one', async () => {
    const { registry } = setUp();

    expect(await registry.invoke(IPC_CHANNELS.componentList)).toEqual([{ path: LISTED, tag: 'hz-card', name: 'Card' }]);
    expect(await registry.invoke(IPC_CHANNELS.componentLastOpened)).toBe(LISTED);
  });

  // covers: FCT-0028
  it('opens a listed component under the listed spelling of its path', async () => {
    const { registry, components } = setUp();

    const opened = await registry.invoke(IPC_CHANNELS.componentOpen, { path: LISTED.toLowerCase() });

    expect(opened).toMatchObject({ path: LISTED, text: 'tag=hz-card' });
    expect(components.calls).toEqual([`resolve ${LISTED.toLowerCase()}`, `open ${LISTED}`]);
  });
});

describe('component request validation', () => {
  it.each([IPC_CHANNELS.componentList, IPC_CHANNELS.componentOpen, IPC_CHANNELS.componentLastOpened])(
    'refuses %s from another sender before reading the request',
    async (channel) => {
      const { registry, components, checkedSenders } = setUp();

      const failure = await rejectionOf(() => registry.invokeFrom({ url: FOREIGN_URL }, channel, { path: LISTED }));

      expect(failure).toMatchObject({ name: 'IpcRejectedError', reason: 'untrusted-sender' });
      expect(checkedSenders).toEqual([FOREIGN_URL]);
      expect(components.calls).toEqual([]);
    },
  );

  it.each([IPC_CHANNELS.componentList, IPC_CHANNELS.componentLastOpened])(
    'refuses a request body on %s',
    async (channel) => {
      const { registry, components } = setUp();

      const failure = await rejectionOf(() => registry.invoke(channel, { path: LISTED }));

      expect(failure).toMatchObject({ reason: 'invalid-payload' });
      expect(components.calls).toEqual([]);
    },
  );

  it.each([
    ['a missing body', undefined],
    ['a string', LISTED],
    ['a number', 42],
    ['a relative path', { path: 'src\\components\\hz-card.ts' }],
    ['an extra key', { path: LISTED, extra: true }],
    ['a nul character', { path: `${LISTED}\u0000` }],
    ['a path that is not listed', { path: 'E:\\kit\\src\\components\\hz-missing.ts' }],
  ])('refuses to open %s, with a generic error', async (_label, payload) => {
    const { registry, components } = setUp();

    const failure = await rejectionOf(() => registry.invoke(IPC_CHANNELS.componentOpen, payload));

    expect(failure).toBeInstanceOf(IpcRejectedError);
    expect(failure).toMatchObject({ message: 'IPC message rejected (invalid-payload)' });
    expect(components.calls.filter((call) => call.startsWith('open'))).toEqual([]);
  });

  it('hides the cause when the listing fails while the request is read', async () => {
    const { registry, components } = setUp();
    components.failResolve = true;

    const failure = await rejectionOf(() => registry.invoke(IPC_CHANNELS.componentOpen, { path: LISTED }));

    expect(failure).toMatchObject({ message: 'The request failed unexpectedly' });
  });

  it('reads an empty payload only when there is none', () => {
    expect(readEmptyPayload(undefined)).toBeUndefined();
    expect(readEmptyPayload(null)).toBeNull();
    expect(readEmptyPayload({})).toBeNull();
  });

  it('reads a listed path through the use cases', async () => {
    const read = readListedComponent(new RecordingComponents());

    expect(await read({ path: LISTED })).toEqual({ path: LISTED });
    expect(await read({ path: 'C:\\Windows\\win.ini' })).toBeNull();
  });
});
