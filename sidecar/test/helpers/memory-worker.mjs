import { isMainThread } from 'node:worker_threads';

if (!isMainThread) {
  const kept = [];
  for (;;) {
    kept.push(new Array(1_000_000).fill(1));
  }
}
