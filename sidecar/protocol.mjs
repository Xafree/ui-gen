export const PROTOCOL_VERSION = 1;
export const SERVICE_NAME = 'uigen-sidecar';
export const MAX_REQUEST_BYTES = 1024 * 1024;

const NEWLINE = 0x0a;

export class ServiceError extends Error {
  constructor(kind, message) {
    super(message);
    this.name = 'ServiceError';
    this.kind = kind;
  }
}

export const greeting = () => ({ protocol: PROTOCOL_VERSION, service: SERVICE_NAME, node: process.versions.node });

export const okResponse = (id, result) => ({ id, ok: true, result });

export const errorResponse = (id, kind, message) => ({ id, ok: false, error: { kind, message } });

export const encodeLine = (message) => `${JSON.stringify(message)}\n`;

const isPlainObject = (value) => typeof value === 'object' && value !== null && !Array.isArray(value);

export function parseRequest(text) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    return { ok: false, id: null, message: 'request is not valid JSON' };
  }
  if (!isPlainObject(parsed)) {
    return { ok: false, id: null, message: 'request must be a JSON object' };
  }
  const { id, method, params = {} } = parsed;
  if (!Number.isSafeInteger(id) || id < 1) {
    return { ok: false, id: null, message: 'request "id" must be an integer >= 1' };
  }
  if (typeof method !== 'string' || method === '') {
    return { ok: false, id, message: 'request "method" must be a non-empty string' };
  }
  if (!isPlainObject(params)) {
    return { ok: false, id, message: 'request "params" must be a JSON object' };
  }
  return { ok: true, request: { id, method, params } };
}

export function createLineReader({ onLine, onOversize, maxBytes = MAX_REQUEST_BYTES }) {
  let chunks = [];
  let size = 0;
  let discarding = false;

  const reset = () => {
    chunks = [];
    size = 0;
  };

  const flush = () => {
    const line = Buffer.concat(chunks).toString('utf8');
    reset();
    if (line.trim() !== '') {
      onLine(line);
    }
  };

  const push = (chunk) => {
    let start = 0;
    while (start < chunk.length) {
      const newline = chunk.indexOf(NEWLINE, start);
      const stop = newline === -1 ? chunk.length : newline;
      if (!discarding) {
        size += stop - start;
        if (size > maxBytes) {
          discarding = true;
          reset();
          onOversize();
        } else {
          chunks.push(chunk.subarray(start, stop));
        }
      }
      if (newline === -1) {
        return;
      }
      if (discarding) {
        discarding = false;
      } else {
        flush();
      }
      start = newline + 1;
    }
  };

  const end = () => {
    if (!discarding && size > 0) {
      flush();
    }
  };

  return { push, end };
}
