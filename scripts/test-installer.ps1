# Lifecycle test of scripts/install.ps1 against a local asset folder (no network), Windows only:
# install, re-run (no-op), user PATH in the registry, checksum mismatch refusal, update while the old
# pkm.exe is running, unmanaged binary refusal, uninstall, and the 'iex' (piped) form.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-installer.ps1 -Fixture <dir> -Version vX.Y.Z
#
# <dir> holds pkm-x86_64-pc-windows-msvc.exe and checksums.txt (scripts/release-assets.sh package).
# Kept ASCII-only, like install.ps1. CI runners are elevated, so PKM_ALLOW_ROOT=1 is set here.
param(
    [Parameter(Mandatory = $true)][string]$Fixture,
    [Parameter(Mandatory = $true)][string]$Version
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$installer = Join-Path $PSScriptRoot 'install.ps1'
$Fixture = (Resolve-Path -LiteralPath $Fixture).ProviderPath
$work = Join-Path ([IO.Path]::GetTempPath()) ('pkm-installer-test-' + [Guid]::NewGuid().ToString('N'))
$bin = Join-Path $work 'bin'
New-Item -ItemType Directory -Path $work | Out-Null
$env:PKM_ALLOW_ROOT = '1'
foreach ($n in 'PKM_VERSION', 'PKM_INSTALL_DIR', 'PKM_UNINSTALL', 'PKM_YES', 'PKM_DRY_RUN', 'PKM_NO_MODIFY_PATH', 'PKM_MCP_INSTALL', 'PKM_LOCAL_FIXTURE') {
    Remove-Item "Env:$n" -ErrorAction SilentlyContinue
}

function Fail([string]$Msg) { throw "FAIL: $Msg" }
function Step([string]$Msg) { Write-Host "== $Msg" }

# Runs install.ps1 as a separate Windows PowerShell process; returns @{ Code; Out }.
function Invoke-Installer([string[]]$InstallerArgs) {
    $all = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $installer) + $InstallerArgs
    $ErrorActionPreference = 'Continue'
    # stdin is redirected, as under CI or a pipe: a confirmation prompt must refuse, not wait.
    $out = '' | & powershell.exe @all 2>&1 | Out-String
    $code = $LASTEXITCODE
    Write-Host $out
    return @{ Code = $code; Out = $out }
}
function Get-UserPath { return [string][Environment]::GetEnvironmentVariable('Path', 'User') }
function Test-UserPathHas([string]$Dir) {
    foreach ($p in ((Get-UserPath) -split ';')) { if ($p.TrimEnd('\') -ieq $Dir.TrimEnd('\')) { return $true } }
    return $false
}

$base = @('-LocalFixture', $Fixture, '-Version', $Version, '-InstallDir', $bin)
$exe = Join-Path $bin 'pkm.exe'
$userPathBefore = Get-UserPath
try {
    Step 'unstamped copy refuses without a version'
    $r = Invoke-Installer @('-LocalFixture', $Fixture, '-InstallDir', $bin)
    if ($r.Code -ne 1 -or $r.Out -notmatch 'has no version') { Fail "unstamped run returned $($r.Code)" }

    Step 'dry run changes nothing'
    $r = Invoke-Installer ($base + '-DryRun')
    if ($r.Code -ne 0 -or (Test-Path -LiteralPath $bin)) { Fail 'dry run wrote files' }

    Step 'install'
    $r = Invoke-Installer $base
    if ($r.Code -ne 0) { Fail "install returned $($r.Code)" }
    $line = (& $exe --version | Select-Object -First 1)
    if (-not $line.StartsWith("pkm $($Version.TrimStart('v')) ")) { Fail "pkm --version printed '$line'" }
    if (-not (Test-UserPathHas $bin)) { Fail 'install dir not added to the user PATH' }

    Step 're-run is a no-op'
    $before = (Get-FileHash -LiteralPath $exe).Hash
    $r = Invoke-Installer $base
    if ($r.Code -ne 0 -or $r.Out -notmatch 'nothing to do') { Fail 're-run did not report nothing to do' }
    $count = @((Get-UserPath) -split ';' | Where-Object { $_.TrimEnd('\') -ieq $bin.TrimEnd('\') }).Count
    if ($count -ne 1) { Fail "install dir is $count times in the user PATH" }

    Step 'checksum mismatch is refused and keeps the old binary'
    $bad = Join-Path $work 'bad'
    New-Item -ItemType Directory -Path $bad | Out-Null
    Copy-Item -Path (Join-Path $Fixture '*') -Destination $bad
    [IO.File]::AppendAllText((Join-Path $bad 'pkm-x86_64-pc-windows-msvc.exe'), 'garbage')
    Set-Content -LiteralPath (Join-Path $bin '.pkm-managed') -Value @('pkm-managed-v1', 'version=v0.0.0') -Encoding Ascii
    $r = Invoke-Installer @('-LocalFixture', $bad, '-Version', $Version, '-InstallDir', $bin)
    if ($r.Code -ne 1 -or $r.Out -notmatch 'checksum mismatch') { Fail "bad checksum returned $($r.Code)" }
    if ((Get-FileHash -LiteralPath $exe).Hash -ne $before) { Fail 'bad checksum replaced the binary' }

    Step 'update while the old pkm.exe is running (as under Claude Desktop)'
    # `pkm VAULT mcp` serves MCP on stdin until it closes, like under Claude Desktop; a process
    # holding the exe open is all that matters here.
    $vault = Join-Path $work 'vault'
    New-Item -ItemType Directory -Path $vault | Out-Null
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = "`"$vault`" mcp"
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $running = [Diagnostics.Process]::Start($psi)
    Start-Sleep -Seconds 2
    if ($running.HasExited) {
        Write-Host "::warning::pkm.exe exited at once (code $($running.ExitCode)); update over a running exe was not exercised"
    }
    try {
        $r = Invoke-Installer $base
        if ($r.Code -ne 0) { Fail "update over a running pkm.exe returned $($r.Code)" }
        if (-not (Select-String -LiteralPath (Join-Path $bin '.pkm-managed') -Pattern "^version=$([regex]::Escape($Version))$" -Quiet)) { Fail 'marker not updated' }
    } finally {
        if (-not $running.HasExited) { $running.Kill() }
        $running.WaitForExit()
    }

    Step 'uninstall without -Yes and without a console is refused'
    $r = Invoke-Installer ($base + '-Uninstall')
    if ($r.Code -ne 1) { Fail "uninstall without -Yes returned $($r.Code)" }

    Step 'uninstall'
    $r = Invoke-Installer ($base + @('-Uninstall', '-Yes'))
    if ($r.Code -ne 0) { Fail "uninstall returned $($r.Code)" }
    if (Test-Path -LiteralPath $exe) { Fail 'pkm.exe still there' }
    if (Test-UserPathHas $bin) { Fail 'install dir still in the user PATH' }

    Step 'piped form (Get-Content | iex) with environment variables'
    $env:PKM_VERSION = $Version; $env:PKM_LOCAL_FIXTURE = $Fixture; $env:PKM_INSTALL_DIR = $bin; $env:PKM_NO_MODIFY_PATH = '1'
    Get-Content -LiteralPath $installer -Raw | Invoke-Expression
    if ($global:LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $exe)) { Fail 'piped install failed' }
    $env:PKM_UNINSTALL = '1'; $env:PKM_YES = '1'
    Get-Content -LiteralPath $installer -Raw | Invoke-Expression
    if ($global:LASTEXITCODE -ne 0 -or (Test-Path -LiteralPath $exe)) { Fail 'piped uninstall failed' }
    foreach ($n in 'PKM_VERSION', 'PKM_LOCAL_FIXTURE', 'PKM_INSTALL_DIR', 'PKM_NO_MODIFY_PATH', 'PKM_UNINSTALL', 'PKM_YES') { Remove-Item "Env:$n" }

    Step 'unmanaged pkm.exe is not overwritten'
    New-Item -ItemType Directory -Path $bin -Force | Out-Null
    [IO.File]::WriteAllText($exe, 'not ours')
    $r = Invoke-Installer $base
    if ($r.Code -ne 1 -or $r.Out -notmatch 'not installed by this script') { Fail "unmanaged overwrite returned $($r.Code)" }
    if ([IO.File]::ReadAllText($exe) -ne 'not ours') { Fail 'unmanaged pkm.exe was changed' }

    Write-Host 'OK: install.ps1 passed every step'
} finally {
    [Environment]::SetEnvironmentVariable('Path', $userPathBefore, 'User')
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
