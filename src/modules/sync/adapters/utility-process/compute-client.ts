import type { AnalyzerStatus, ComponentAnalysis, ComponentAnalyzer } from '../../ports/component-analyzer.js';
import type { ComponentModel } from '../../domain/model/component-model.js';
import { readComputeMessage, type ComputeReply, type ComputeRequest } from './compute-protocol.js';

type DistributiveOmit<Union, Key extends PropertyKey> = Union extends unknown ? Omit<Union, Key> : never;

export interface ComputeProcess {
  send(request: ComputeRequest): void;
  onMessage(listener: (message: unknown) => void): void;
  onExit(listener: () => void): void;
  kill(): void;
}

type RequestContent = DistributiveOmit<ComputeRequest, 'id'>;

export interface ComputeClientOptions {
  readonly spawn: () => ComputeProcess;
  readonly onStatus: (status: AnalyzerStatus) => void;
  readonly requestTimeoutMs?: number;
  readonly restartBaseDelayMs?: number;
  readonly restartMaxDelayMs?: number;
}

export class ComputeUnavailableError extends Error {
  constructor(reason: string) {
    super(`The analysis process is unavailable (${reason})`);
    this.name = 'ComputeUnavailableError';
  }
}

interface PendingRequest {
  readonly process: ComputeProcess;
  readonly resolve: (reply: ComputeReply) => void;
  readonly reject: (error: Error) => void;
  readonly timer: ReturnType<typeof setTimeout>;
}

const DEFAULT_REQUEST_TIMEOUT_MS = 10_000;
const DEFAULT_RESTART_BASE_DELAY_MS = 100;
const DEFAULT_RESTART_MAX_DELAY_MS = 5_000;

export class ComputeClient implements ComponentAnalyzer {
  readonly #options: ComputeClientOptions;
  readonly #pending = new Map<number, PendingRequest>();
  #ready: ComputeProcess | null = null;
  #waiters: ((process: ComputeProcess) => void)[] = [];
  #starting: ComputeProcess | null = null;
  #restartTimer: ReturnType<typeof setTimeout> | null = null;
  #failures = 0;
  #lastId = 0;
  #stopped = true;

  constructor(options: ComputeClientOptions) {
    this.#options = options;
  }

  start(): void {
    if (!this.#stopped) return;
    this.#stopped = false;
    this.#launch();
  }

  stop(): void {
    this.#stopped = true;
    if (this.#restartTimer !== null) clearTimeout(this.#restartTimer);
    this.#restartTimer = null;
    const running = this.#ready ?? this.#starting;
    this.#ready = null;
    this.#starting = null;
    this.#waiters = [];
    this.#rejectPending(null, 'stopped');
    running?.kill();
  }

  async readModel(text: string): Promise<ComponentModel> {
    const reply = await this.#request({ operation: 'read-model', text });
    if (reply.outcome !== 'model') throw new ComputeUnavailableError('unexpected reply');
    return reply.model;
  }

  async analyze(text: string): Promise<ComponentAnalysis> {
    const reply = await this.#request({ operation: 'analyze', text });
    if (reply.outcome !== 'analysis') throw new ComputeUnavailableError('unexpected reply');
    return { model: reply.model, highlight: reply.highlight };
  }

  async transpile(source: string, tsconfigText: string | null, fileName: string): Promise<string> {
    const reply = await this.#request({ operation: 'transpile', text: source, tsconfig: tsconfigText, fileName });
    if (reply.outcome !== 'transpiled') throw new ComputeUnavailableError('unexpected reply');
    return reply.code;
  }

  #launch(): void {
    this.#restartTimer = null;
    let process: ComputeProcess;
    try {
      process = this.#options.spawn();
    } catch (cause) {
      console.error('The analysis process cannot be started', cause);
      this.#scheduleRestart();
      return;
    }
    this.#starting = process;
    process.onMessage((message) => {
      this.#receive(process, message);
    });
    process.onExit(() => {
      this.#lose(process);
    });
  }

  #receive(process: ComputeProcess, message: unknown): void {
    const reply = readComputeMessage(message);
    if (reply === null) {
      console.warn('The analysis process sent an unreadable message');
      return;
    }
    if (reply.outcome === 'ready') {
      this.#markReady(process);
      return;
    }
    const pending = this.#pending.get(reply.id);
    if (pending?.process !== process) return;
    this.#pending.delete(reply.id);
    clearTimeout(pending.timer);
    if (reply.outcome === 'failed') pending.reject(new ComputeUnavailableError('analysis failed'));
    else pending.resolve(reply);
  }

  #markReady(process: ComputeProcess): void {
    if (this.#stopped || this.#starting !== process) return;
    this.#starting = null;
    this.#ready = process;
    this.#failures = 0;
    const waiters = this.#waiters;
    this.#waiters = [];
    for (const waiter of waiters) waiter(process);
    this.#options.onStatus('ready');
  }

  #lose(process: ComputeProcess): void {
    if (this.#stopped || (this.#ready !== process && this.#starting !== process)) return;
    this.#ready = null;
    this.#starting = null;
    this.#rejectPending(process, 'exited');
    this.#options.onStatus('restarting');
    this.#scheduleRestart();
  }

  #scheduleRestart(): void {
    const base = this.#options.restartBaseDelayMs ?? DEFAULT_RESTART_BASE_DELAY_MS;
    const max = this.#options.restartMaxDelayMs ?? DEFAULT_RESTART_MAX_DELAY_MS;
    const delay = Math.min(base * 2 ** this.#failures, max);
    this.#failures += 1;
    this.#restartTimer = setTimeout(() => {
      this.#launch();
    }, delay);
  }

  #rejectPending(process: ComputeProcess | null, reason: string): void {
    for (const [id, pending] of this.#pending) {
      if (process !== null && pending.process !== process) continue;
      this.#pending.delete(id);
      clearTimeout(pending.timer);
      pending.reject(new ComputeUnavailableError(reason));
    }
  }

  #whenReady(): Promise<ComputeProcess> {
    if (this.#ready !== null) return Promise.resolve(this.#ready);
    if (this.#stopped) return Promise.reject(new ComputeUnavailableError('stopped'));
    const timeoutMs = this.#options.requestTimeoutMs ?? DEFAULT_REQUEST_TIMEOUT_MS;
    return new Promise((resolve, reject) => {
      const waiter = (process: ComputeProcess): void => {
        clearTimeout(timer);
        resolve(process);
      };
      const timer = setTimeout(() => {
        this.#waiters = this.#waiters.filter((candidate) => candidate !== waiter);
        reject(new ComputeUnavailableError('not ready'));
      }, timeoutMs);
      this.#waiters.push(waiter);
    });
  }

  async #request(content: RequestContent): Promise<ComputeReply> {
    const process = await this.#whenReady();
    const id = ++this.#lastId;
    const timeoutMs = this.#options.requestTimeoutMs ?? DEFAULT_REQUEST_TIMEOUT_MS;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#pending.delete(id);
        reject(new ComputeUnavailableError('timed out'));
        process.kill();
      }, timeoutMs);
      this.#pending.set(id, { process, resolve, reject, timer });
      try {
        process.send({ id, ...content });
      } catch (cause) {
        console.error('A request cannot reach the analysis process', cause);
        this.#pending.delete(id);
        clearTimeout(timer);
        reject(new ComputeUnavailableError('unreachable'));
      }
    });
  }
}
