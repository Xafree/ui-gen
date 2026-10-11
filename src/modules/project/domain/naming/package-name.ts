import { toSlug } from './slug.js';

const PACKAGE_NAME_PATTERN = /^(@[a-z0-9][a-z0-9._-]*\/)?[a-z0-9][a-z0-9._-]*$/;
const UI_SUFFIX = '/ui';

export type PackageNameError = 'empty' | 'invalid';

export function suggestPackageName(projectName: string): string {
  const slug = toSlug(projectName);
  return slug === '' ? '' : `@${slug}${UI_SUFFIX}`;
}

export function validatePackageName(value: string): PackageNameError | null {
  if (value === '') return 'empty';
  return PACKAGE_NAME_PATTERN.test(value) ? null : 'invalid';
}

export function projectFolderName(packageName: string): string {
  const withoutAt = packageName.startsWith('@') ? packageName.slice(1) : packageName;
  const withoutUi = withoutAt.endsWith(UI_SUFFIX) ? withoutAt.slice(0, -UI_SUFFIX.length) : withoutAt;
  return withoutUi.replaceAll('/', '-');
}
