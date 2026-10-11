const DECLARATION = /(?:customElements\.define|@customElement)\(\s*(['"`])([a-z][a-z0-9]*-[a-z0-9-]*)\1/g;

export function findDeclaredTags(source: string): string[] {
  return Array.from(source.matchAll(DECLARATION), (match) => match[2]).filter((tag) => tag !== undefined);
}
