import { loadBundle, withPage } from './browser.mjs';
import { IS_COMPONENT, bundleEntry } from './bundle.mjs';
import { readInstalledPackage } from './installed-package.mjs';
import { ServiceError } from './protocol.mjs';

const INSPECT_TIMEOUT_MS = 55_000;
const EMPTY_PAGE = '<!doctype html><meta charset="utf-8"><body></body>';

const byName = (a, b) => (a.name < b.name ? -1 : Number(a.name > b.name));

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
    return page.evaluate(() => window.__uigenExports);
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
