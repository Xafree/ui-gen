import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  ComputeClient,
  ComputeUnavailableError,
  type ComputeProcess,
} from '../../../../../src/modules/sync/adapters/utility-process/compute-client';
import {
  MAX_COMPUTE_TEXT_LENGTH,
  readComputeMessage,
  readComputeRequest,
  type ComputeRequest,
} from '../../../../../src/modules/sync/adapters/utility-process/compute-protocol';
import {
  createTranspileServer,
  routeComputeRequests,
} from '../../../../../src/modules/sync/adapters/utility-process/compute-server';
import { modelOf } from '../../application/components/component-doubles';

afterEach(() => {
  vi.restoreAllMocks();
});

const VALID = { id: 1, operation: 'transpile', text: 'a', tsconfig: null, fileName: 'a.ts' };

describe('transpile requests', () => {
  it('reads a transpile request with or without a tsconfig', () => {
    expect(readComputeRequest({ ...VALID, tsconfig: '{}' })).toEqual({ ...VALID, tsconfig: '{}' });
    expect(readComputeRequest(VALID)).toEqual(VALID);
  });

  it.each([
    ['no file name', { ...VALID, fileName: undefined }],
    ['an empty file name', { ...VALID, fileName: '' }],
    ['a file name that is too long', { ...VALID, fileName: 'x'.repeat(1025) }],
    ['no tsconfig field', { ...VALID, tsconfig: undefined }],
    ['a tsconfig that is not text', { ...VALID, tsconfig: 3 }],
    ['a tsconfig that is too long', { ...VALID, tsconfig: 'x'.repeat(100_001) }],
    ['a source that is too long', { ...VALID, text: 'x'.repeat(MAX_COMPUTE_TEXT_LENGTH + 1) }],
    ['a bad id', { ...VALID, id: -1 }],
  ])('refuses %s', (_label, value) => {
    expect(readComputeRequest(value)).toBeNull();
  });

  it('accepts a tsconfig and a file name of exactly the maximum length', () => {
    const request = { ...VALID, tsconfig: 'x'.repeat(100_000), fileName: 'y'.repeat(1024) };

    expect(readComputeRequest(request)).toEqual(request);
  });

  it('reads the transpiled reply and refuses one without code', () => {
    expect(readComputeMessage({ outcome: 'transpiled', id: 2, code: 'x' })).toEqual({
      outcome: 'transpiled',
      id: 2,
      code: 'x',
    });
    expect(readComputeMessage({ outcome: 'transpiled', id: 2 })).toBeNull();
    expect(readComputeMessage({ outcome: 'transpiled', id: 2, code: 4 })).toBeNull();
  });
});

describe('transpile server', () => {
  const request = { id: 7, operation: 'transpile', text: 'src', tsconfig: '{}', fileName: 'a.ts' } as const;

  it('answers the compiled code', () => {
    const calls: unknown[][] = [];
    const serve = createTranspileServer((...args) => {
      calls.push(args);
      return 'out';
    });

    expect(serve(request)).toEqual({ outcome: 'transpiled', id: 7, code: 'out' });
    expect(calls).toEqual([['src', '{}', 'a.ts']]);
  });

  it('answers a failure, and logs it, when the module cannot be compiled', () => {
    const logged = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const serve = createTranspileServer(() => {
      throw new Error('syntax');
    });

    expect(serve(request)).toEqual({ outcome: 'failed', id: 7 });
    expect(logged).toHaveBeenCalledOnce();
  });

  it('routes each request to its own server', () => {
    const route = routeComputeRequests(
      ({ id }) => ({ outcome: 'failed', id: id + 100 }),
      ({ id }) => ({ outcome: 'transpiled', id, code: '' }),
    );

    expect(route({ id: 1, operation: 'analyze', text: '' })).toEqual({ outcome: 'failed', id: 101 });
    expect(route(request)).toEqual({ outcome: 'transpiled', id: 7, code: '' });
  });
});

class Process implements ComputeProcess {
  readonly sent: ComputeRequest[] = [];
  #message: (message: unknown) => void = () => undefined;

  send(request: ComputeRequest): void {
    this.sent.push(request);
  }

  onMessage(listener: (message: unknown) => void): void {
    this.#message = listener;
  }

  onExit(): void {
    return undefined;
  }

  kill(): void {
    return undefined;
  }

  say(message: unknown): void {
    this.#message(message);
  }
}

describe('transpiling through the compute client', () => {
  function startClient(): { client: ComputeClient; process: Process } {
    const process = new Process();
    const client = new ComputeClient({ spawn: () => process, onStatus: () => undefined });
    client.start();
    process.say({ outcome: 'ready' });
    return { client, process };
  }

  it('sends the source, the tsconfig and the file name, and resolves with the code', async () => {
    const { client, process } = startClient();

    const pending = client.transpile('src', null, 'a.ts');
    await vi.waitFor(() => {
      expect(process.sent).toHaveLength(1);
    });
    process.say({ outcome: 'transpiled', id: process.sent[0]?.id, code: 'out' });

    await expect(pending).resolves.toBe('out');
    expect(process.sent[0]).toMatchObject({ operation: 'transpile', text: 'src', tsconfig: null, fileName: 'a.ts' });
    client.stop();
  });

  it('rejects when the compute process fails the module or answers something else', async () => {
    const { client, process } = startClient();

    const failing = client.transpile('src', null, 'a.ts');
    await vi.waitFor(() => {
      expect(process.sent).toHaveLength(1);
    });
    process.say({ outcome: 'failed', id: process.sent[0]?.id });
    await expect(failing).rejects.toThrow('The analysis process is unavailable (analysis failed)');

    const confused = client.transpile('src', null, 'a.ts');
    await vi.waitFor(() => {
      expect(process.sent).toHaveLength(2);
    });
    process.say({ outcome: 'model', id: process.sent[1]?.id, model: modelOf('tag=hz-a') });
    await expect(confused).rejects.toBeInstanceOf(ComputeUnavailableError);
    client.stop();
  });
});
