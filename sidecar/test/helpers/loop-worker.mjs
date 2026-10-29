import { isMainThread } from 'node:worker_threads';

if (!isMainThread) {
  for (;;) {
    // never yields: only terminate() ends it
  }
}
