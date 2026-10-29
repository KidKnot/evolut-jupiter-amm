import { execSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const args = process.argv.slice(2);
if (!['check', 'test', 'fmt', 'clippy', 'fetch', 'generate-lockfile'].includes(args[0])) {
  throw new Error('Use check, test, fmt, clippy, fetch, or generate-lockfile. This tool never deploys.');
}
const env = { ...process.env, RUSTUP_TOOLCHAIN: 'stable' };
if (process.platform === 'win32' && !env.VCToolsInstallDir) {
  const setup = 'C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\Common7\\Tools\\VsDevCmd.bat';
  const configured = execSync(`call "${setup}" -arch=amd64 -host_arch=amd64 >nul && set`, { encoding:'utf8', windowsHide:true, shell:'cmd.exe' });
  for (const line of configured.split(/\r?\n/)) {
    const separator = line.indexOf('=');
    if (separator > 0) env[line.slice(0, separator)] = line.slice(separator + 1);
  }
}
// VsDevCmd's environment may contain an inherited toolchain override.
env.RUSTUP_TOOLCHAIN = 'stable';
const result = spawnSync('cargo', args, { cwd:fileURLToPath(new URL('.', import.meta.url)), env, stdio:'inherit', windowsHide:true });
if (result.error) throw result.error;
process.exit(result.status ?? 1);
