import { execFileSync } from 'node:child_process';

const MAX_BUFFER = 64 * 1024 * 1024;

function listProcesses() {
  if (process.platform === 'win32') {
    const command = 'Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,CommandLine | ConvertTo-Json -Compress';
    const output = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', command], {
      encoding: 'utf8',
      windowsHide: true,
      maxBuffer: MAX_BUFFER,
    });
    return JSON.parse(output).map((entry) => ({ pid: entry.ProcessId, ppid: entry.ParentProcessId, command: entry.CommandLine ?? '' }));
  }
  const output = execFileSync('ps', ['-eo', 'pid=,ppid=,args='], { encoding: 'utf8', maxBuffer: MAX_BUFFER });
  return output
    .split('\n')
    .map((line) => /^\s*(\d+)\s+(\d+)\s+(.*)$/.exec(line))
    .filter((match) => match !== null)
    .map((match) => ({ pid: Number(match[1]), ppid: Number(match[2]), command: match[3] }));
}

export function killRenderersUnder(rootPid) {
  const all = listProcesses();
  const family = new Set([rootPid]);
  for (let grew = true; grew; ) {
    grew = false;
    for (const entry of all) {
      if (!family.has(entry.pid) && family.has(entry.ppid)) {
        family.add(entry.pid);
        grew = true;
      }
    }
  }
  const renderers = all.filter((entry) => family.has(entry.pid) && entry.command.includes('--type=renderer'));
  for (const { pid } of renderers) {
    try {
      process.kill(pid);
    } catch {
      continue;
    }
  }
  return renderers.length;
}
