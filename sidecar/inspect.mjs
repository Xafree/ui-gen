import { loadBundle, withPage } from './browser.mjs';
import { IS_COMPONENT, bundleEntry } from './bundle.mjs';
import { readInstalledPackage } from './installed-package.mjs';
import { ServiceError } from './protocol.mjs';

const INSPECT_TIMEOUT_MS = 55_000;
const EMPTY_PAGE = '<!doctype html><meta charset="utf-8"><body></body>';

const MAX_EXPORTS = 10_000;
const MAX_EXPORT_NAME_LENGTH = 128;

const byName = (a, b) => (a.name < b.name ? -1 : Number(a.name > b.name));

export function checkExportNames(value, packageName) {
  const valid =
    Array.isArray(value) &&
    value.length <= MAX_EXPORTS &&
    value.every((name) => typeof name === 'string' && name.length > 0 && name.length <= MAX_EXPORT_NAME_LENGTH);
  if (!valid) {
    throw new ServiceError('render', `${packageName} reported an invalid list of exports (at most ${MAX_EXPORTS} names of ${MAX_EXPORT_NAME_LENGTH} characters)`);
  }
  return value;
}

async function listReactExports(installed) {
  const code = await bundleEntry({
    resolveDir: installed.sources,
    contents: `import * as lib from ${JSON.stringify(installed.name)};
const isComponent = ${IS_COMPONENT};
window.__uigenExports = Object.keys(lib).filter((key) => /^[A-Z]/.test(key) && isComponent(lib[key]));`,
  });
  return withPage({}, async (page) => {
    page.setDefaultTimeout(INSPECT_TIMEOUT_MS);
    const errors = await loadBundle(page, { code, html: EMPTY_PAGE });
    if (errors.length > 0) {
      throw new ServiceError('render', `${installed.name} throws when it loads: ${errors[0]}`);
    }
    return checkExportNames(await page.evaluate(() => window.__uigenExports), installed.name);
  });
}

export async function inspect(params) {
  const installed = await readInstalledPackage(params.dir, params.package);
  const exported = installed.framework === 'react' ? await listReactExports(installed) : [];
  const names = [...new Set([...exported, ...installed.tags.keys()])];
  return {
    package: installed.name,
    version: installed.version,
    framework: installed.framework,
    components: names.map((name) => ({ name, props: [] })).sort(byName),
  };
}
