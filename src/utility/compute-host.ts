import { readComponent } from '../modules/sync/application/read-component.js';
import { readComputeRequest } from '../modules/sync/adapters/utility-process/compute-protocol.js';
import { createComputeServer } from '../modules/sync/adapters/utility-process/compute-server.js';
import { createTypeScriptComponentParser } from '../modules/sync/adapters/typescript/typescript-component-parser.js';
import { createTypeScriptHighlighter } from '../modules/sync/adapters/typescript/typescript-highlighter.js';

const parentPort = process.parentPort;
const parser = createTypeScriptComponentParser();
const serve = createComputeServer({
  readModel: (text) => readComponent(text, parser),
  highlighter: createTypeScriptHighlighter(),
});

parentPort.on('message', ({ data }) => {
  const request = readComputeRequest(data);
  if (request === null) {
    console.warn('The analysis process refused an unreadable request');
    return;
  }
  parentPort.postMessage(serve(request));
});

parentPort.postMessage({ outcome: 'ready' });
