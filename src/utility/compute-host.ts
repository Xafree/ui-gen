import { transpileProjectModule } from '../modules/canvas/adapters/typescript/typescript-transpiler.js';
import { readComponent } from '../modules/sync/application/read-component.js';
import { readComputeRequest, refusalOf } from '../modules/sync/adapters/utility-process/compute-protocol.js';
import {
  createComputeServer,
  createTranspileServer,
  routeComputeRequests,
} from '../modules/sync/adapters/utility-process/compute-server.js';
import { createTypeScriptComponentParser } from '../modules/sync/adapters/typescript/typescript-component-parser.js';
import { createTypeScriptHighlighter } from '../modules/sync/adapters/typescript/typescript-highlighter.js';

const parentPort = process.parentPort;
const parser = createTypeScriptComponentParser();
const serve = routeComputeRequests(
  createComputeServer({
    readModel: (text) => readComponent(text, parser),
    highlighter: createTypeScriptHighlighter(),
  }),
  createTranspileServer(transpileProjectModule),
);

parentPort.on('message', ({ data }) => {
  const request = readComputeRequest(data);
  if (request === null) {
    console.warn('The analysis process refused an unreadable request');
    const refusal = refusalOf(data);
    if (refusal !== null) parentPort.postMessage(refusal);
    return;
  }
  parentPort.postMessage(serve(request));
});

parentPort.postMessage({ outcome: 'ready' });
