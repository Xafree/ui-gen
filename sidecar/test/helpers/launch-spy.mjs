import { appendFileSync } from 'node:fs';
import { chromium } from 'playwright';

const logFile = process.env.UIGEN_LAUNCH_LOG;
const record = (entry) => appendFileSync(logFile, `${JSON.stringify(entry)}\n`);
const launch = chromium.launch.bind(chromium);

chromium.launch = async (options) => {
  record({ event: 'launch', options: options ?? null });
  const browser = await launch(options);
  browser.on('disconnected', () => record({ event: 'disconnected' }));
  return browser;
};
