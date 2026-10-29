const MAX_NAME_BYTES = 214;
const MAX_VERSION_BYTES = 64;
const NAME_PART = '[a-z0-9~][a-z0-9-._~]*';
const PACKAGE_NAME = new RegExp(`^(?:@${NAME_PART}/)?${NAME_PART}$`);
const EXACT_VERSION =
  /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

const refuse = (reason) => ({ ok: false, reason });

export function validatePackageName(name) {
  if (typeof name !== 'string') {
    return refuse('package name must be a string');
  }
  if (name === '') {
    return refuse('package name is empty');
  }
  if (Buffer.byteLength(name) > MAX_NAME_BYTES) {
    return refuse(`package name is longer than ${MAX_NAME_BYTES} bytes`);
  }
  if (name.startsWith('-')) {
    return refuse('package name must not start with "-"');
  }
  if (name.endsWith('.tgz')) {
    return refuse('package name must be a registry name, not an archive');
  }
  if (!PACKAGE_NAME.test(name)) {
    return refuse('package name must be a lowercase registry name: "name" or "@scope/name"');
  }
  return { ok: true };
}

export function validateVersion(version, { allowLatest = false } = {}) {
  if (typeof version !== 'string') {
    return refuse('version must be a string');
  }
  if (Buffer.byteLength(version) > MAX_VERSION_BYTES) {
    return refuse(`version is longer than ${MAX_VERSION_BYTES} bytes`);
  }
  if (version === 'latest') {
    return allowLatest ? { ok: true } : refuse('version "latest" is not allowed here, give an exact version');
  }
  if (!EXACT_VERSION.test(version)) {
    return refuse('version must be an exact semver MAJOR.MINOR.PATCH, no range or tag');
  }
  return { ok: true };
}
