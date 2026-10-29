if (typeof process !== 'undefined' && typeof process.getBuiltinModule === 'function') {
  process.getBuiltinModule('node:fs').writeFileSync(process.env.UIGEN_TRIPWIRE, 'executed by node');
}
