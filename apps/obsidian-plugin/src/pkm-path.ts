// Where the installers put the pkm binary (scripts/install.ps1, scripts/install.sh), in the order to try.
import path from 'node:path';

export interface HostInfo {
  platform: NodeJS.Platform;
  env: Record<string, string | undefined>;
  home: string;
}

/** Candidate binary paths: PKM_INSTALL_DIR first (both installers honour it), then each installer's default. */
export function candidatePaths(host: HostInfo): string[] {
  const win = host.platform === 'win32';
  const p = win ? path.win32 : path.posix;
  const exe = win ? 'pkm.exe' : 'pkm';
  const out: string[] = [];
  const custom = host.env.PKM_INSTALL_DIR?.trim();
  if (custom) out.push(p.join(custom, exe));
  if (win) {
    const local = host.env.LOCALAPPDATA?.trim();
    if (local) out.push(p.join(local, 'pkm', 'bin', exe));
  } else if (host.home) {
    out.push(p.join(host.home, '.local', 'bin', exe));
  }
  return out;
}

/** First candidate that exists, or null. */
export function detectPkm(host: HostInfo, exists: (file: string) => boolean): string | null {
  return candidatePaths(host).find((f) => exists(f)) ?? null;
}
