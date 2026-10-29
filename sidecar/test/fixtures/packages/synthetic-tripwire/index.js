const nodeFs = globalThis.process?.getBuiltinModule?.('node:fs');
const witness = globalThis.process?.env?.UIGEN_WITNESS;
if (nodeFs !== undefined && witness !== undefined) {
  nodeFs.writeFileSync(witness, 'executed by node');
}

export function Tripwire() {
  return null;
}
