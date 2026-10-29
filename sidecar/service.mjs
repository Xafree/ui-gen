import { closeBrowser } from './browser.mjs';
import { inspect } from './inspect.mjs';
import { install, uninstall } from './install.mjs';
import { render } from './render.mjs';
import { killRunningNpm, runNpm } from './run-npm.mjs';
import { ServiceError, createLineReader, encodeLine, errorResponse, greeting, okResponse, parseRequest } from './protocol.mjs';

export const methods = new Map();

methods.set('shutdown', async () => {
  await closeBrowser();
  return {};
});
methods.set('install', (params) => install(params, { runNpm }));
methods.set('uninstall', (params) => uninstall(params, { runNpm }));
methods.set('inspect', (params) => inspect(params));
methods.set('render', (params) => render(params));

const inFlight = new Set();
let stopping = false;

const send = (message, done) => process.stdout.write(encodeLine(message), done);

process.on('exit', killRunningNpm);

const stop = () => {
  if (stopping) {
    return;
  }
  stopping = true;
  Promise.allSettled([...inFlight])
    .then(closeBrowser)
    .finally(() => process.stdout.write('', () => process.exit(0)));
};

async function dispatch({ id, method, params }) {
  const handler = methods.get(method);
  if (handler === undefined) {
    return errorResponse(id, 'invalidRequest', `unknown method "${method}"`);
  }
  try {
    return okResponse(id, await handler(params));
  } catch (error) {
    if (error instanceof ServiceError) {
      return errorResponse(id, error.kind, error.message);
    }
    process.stderr.write(`internal error in "${method}": ${error instanceof Error ? error.name : 'unknown'}\n`);
    return errorResponse(id, 'internal', `internal error in "${method}"`);
  }
}

function handleLine(line) {
  const parsed = parseRequest(line);
  if (!parsed.ok) {
    send(errorResponse(parsed.id, 'invalidRequest', parsed.message));
    return;
  }
  const { request } = parsed;
  const work = dispatch(request).then((response) => {
    if (request.method === 'shutdown' && response.ok) {
      send(response, () => process.exit(0));
      stopping = true;
    } else {
      send(response);
    }
  });
  inFlight.add(work);
  work.finally(() => inFlight.delete(work));
}

const reader = createLineReader({
  onLine: handleLine,
  onOversize: () => send(errorResponse(null, 'tooLarge', 'request line is larger than 1 MiB and was ignored')),
});

send(greeting());
process.stdin.on('data', (chunk) => reader.push(chunk));
process.stdin.on('end', () => {
  reader.end();
  stop();
});
