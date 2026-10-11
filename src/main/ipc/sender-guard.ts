import { fileURLToPath } from 'node:url';

type SenderGuard = (senderUrl: string) => boolean;

const WEB_PROTOCOLS: ReadonlySet<string> = new Set(['http:', 'https:']);
const FILE_PROTOCOL = 'file:';

function parseUrl(text: string): URL | null {
  return URL.canParse(text) ? new URL(text) : null;
}

function windowsPathOf(fileUrl: URL): string {
  return fileURLToPath(fileUrl, { windows: true }).toLowerCase();
}

function windowsPathOrNull(url: URL): string | null {
  if (url.protocol !== FILE_PROTOCOL) return null;
  try {
    return windowsPathOf(url);
  } catch {
    return null;
  }
}

function createOriginGuard({ protocol, host }: URL): SenderGuard {
  return (senderUrl) => {
    const sender = parseUrl(senderUrl);
    return sender !== null && sender.protocol === protocol && sender.host === host;
  };
}

function createFileGuard(renderer: URL): SenderGuard {
  const trustedPath = windowsPathOf(renderer);
  return (senderUrl) => {
    const sender = parseUrl(senderUrl);
    return sender !== null && windowsPathOrNull(sender) === trustedPath;
  };
}

export function createSenderGuard(rendererUrl: string): SenderGuard {
  const renderer = new URL(rendererUrl);
  if (WEB_PROTOCOLS.has(renderer.protocol)) return createOriginGuard(renderer);
  if (renderer.protocol === FILE_PROTOCOL) return createFileGuard(renderer);
  throw new TypeError(`Unsupported renderer URL protocol ${renderer.protocol}`);
}
