import { execFileSync } from 'node:child_process';

const MAX_BUFFER = 64 * 1024 * 1024;

function listProcesses() {
  if (process.platform === 'win32') {
    const command = 'Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId | ConvertTo-Json -Compress';
    const output = execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', command], {
      encoding: 'utf8',
      windowsHide: true,
      maxBuffer: MAX_BUFFER,
    });
    return JSON.parse(output).map((entry) => ({ pid: entry.ProcessId, ppid: entry.ParentProcessId }));
  }
  const output = execFileSync('ps', ['-eo', 'pid=,ppid='], { encoding: 'utf8', maxBuffer: MAX_BUFFER });
  return output
    .split('\n')
    .map((line) => /^\s*(\d+)\s+(\d+)\s*$/.exec(line))
    .filter((match) => match !== null)
    .map((match) => ({ pid: Number(match[1]), ppid: Number(match[2]) }));
}

export function descendantsOf(rootPid) {
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
  family.delete(rootPid);
  return [...family];
}

export function isAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}
