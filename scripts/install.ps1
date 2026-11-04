<#
.SYNOPSIS
    Install, update or remove pkm on Windows (PowerShell 5.1 or later; no admin, no Git Bash, no gh, no Python).

.DESCRIPTION
    irm https://github.com/vantt/obsidian-pkm/releases/latest/download/install.ps1 | iex

    A pre-release is never 'latest': name its tag instead, e.g.
    irm https://github.com/vantt/obsidian-pkm/releases/download/v0.1.0-rc.4/install.ps1 | iex

    Each asset is tried anonymously first; on HTTP 401/403/404 (a private fork set with -Repo) it
    falls back to 'gh release download' (needs the GitHub CLI, logged in with 'gh auth login').
    Downloads pkm-x86_64-pc-windows-msvc.exe and checksums.txt of one release, refuses unless the
    binary's own checksums.txt line matches, runs it once, then puts it at
    %LOCALAPPDATA%\pkm\bin\pkm.exe and adds that folder to the user PATH.
    Re-running updates in place (a running pkm.exe, e.g. started by Claude Desktop, is renamed to
    pkm.exe.old-* and removed on a later run); the same version is a no-op.

    Options, as parameters or environment variables (piped into iex only the variables work):
      -Version vX.Y.Z      PKM_VERSION          version to install (default: this release)
      -InstallDir DIR      PKM_INSTALL_DIR      default: %LOCALAPPDATA%\pkm\bin
      -McpInstall VAULT    PKM_MCP_INSTALL      then run 'pkm mcp install VAULT' (quit Claude Desktop first)
      -NoModifyPath        PKM_NO_MODIFY_PATH=1 only print how to add the folder to PATH
      -DryRun              PKM_DRY_RUN=1        show what would happen, change nothing
      -Uninstall           PKM_UNINSTALL=1      remove pkm.exe and the PATH entry (keeps index and data)
      -Yes                 PKM_YES=1            do not ask for confirmation
      -Repo OWNER/NAME     PKM_RELEASE_REPO     GitHub repository of the releases (default: vantt/obsidian-pkm)
      -ReleaseBaseUrl URL  PKM_RELEASE_BASE_URL https URL holding download/<tag>/<asset>
                                                (default: https://github.com/<repo>/releases)
      -LocalFixture DIR    PKM_LOCAL_FIXTURE    take assets from DIR instead of downloading (tests)
    Other: PKM_ALLOW_ROOT=1 (allow an elevated session).

    With parameters:
      & ([scriptblock]::Create((irm https://github.com/vantt/obsidian-pkm/releases/latest/download/install.ps1))) -McpInstall "D:\Vault"
    Uninstall:
      $env:PKM_UNINSTALL = '1'; irm https://github.com/vantt/obsidian-pkm/releases/latest/download/install.ps1 | iex

    Kept ASCII-only on purpose: Windows PowerShell 5.1 decodes 'irm' output and BOM-less .ps1
    files with a legacy code page.
#>
[CmdletBinding()]
param(
    [string]$Version = '',
    [string]$InstallDir = '',
    [string]$McpInstall = '',
    [string]$LocalFixture = '',
    [string]$Repo = '',
    [string]$ReleaseBaseUrl = '',
    [switch]$Uninstall,
    [switch]$NoModifyPath,
    [switch]$DryRun,
    [switch]$Yes
)

function Invoke-PkmInstaller {
    param(
        [string]$Version, [string]$InstallDir, [string]$McpInstall, [string]$LocalFixture,
        [string]$Repo, [string]$ReleaseBaseUrl, [bool]$Uninstall, [bool]$NoModifyPath, [bool]$DryRun, [bool]$Yes
    )
    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'

    $defaultVersion = '@PKM_VERSION@'   # stamped by the release workflow
    $target = 'x86_64-pc-windows-msvc'
    $asset = "pkm-$target.exe"            # same rule as scripts/release-assets.sh asset_name
    $exeName = 'pkm.exe'
    $markerName = '.pkm-managed'
    $maxBytes = 134217728                 # 128 MiB per download

    function Get-Opt([string]$Param, [string]$EnvName) {
        if ($Param) { return $Param }
        $v = [Environment]::GetEnvironmentVariable($EnvName)
        if ($v) { return $v }
        return ''
    }
    function Get-Flag([bool]$Param, [string]$EnvName) {
        return ($Param -or ([Environment]::GetEnvironmentVariable($EnvName) -eq '1'))
    }

    $Version = Get-Opt $Version 'PKM_VERSION'
    if (-not $Version) { $Version = $defaultVersion }
    $InstallDir = Get-Opt $InstallDir 'PKM_INSTALL_DIR'
    $McpInstall = Get-Opt $McpInstall 'PKM_MCP_INSTALL'
    $LocalFixture = Get-Opt $LocalFixture 'PKM_LOCAL_FIXTURE'
    $Uninstall = Get-Flag $Uninstall 'PKM_UNINSTALL'
    $NoModifyPath = Get-Flag $NoModifyPath 'PKM_NO_MODIFY_PATH'
    $DryRun = Get-Flag $DryRun 'PKM_DRY_RUN'
    $Yes = Get-Flag $Yes 'PKM_YES'
    $repo = Get-Opt $Repo 'PKM_RELEASE_REPO'
    if (-not $repo) { $repo = 'vantt/obsidian-pkm' }
    if ($repo -notmatch '^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$') { throw "invalid -Repo: $repo (expected owner/name)" }
    $releaseBase = Get-Opt $ReleaseBaseUrl 'PKM_RELEASE_BASE_URL'
    if (-not $releaseBase) { $releaseBase = "https://github.com/$repo/releases" }
    $releaseBase = $releaseBase.TrimEnd('/')

    if ($PSVersionTable.PSVersion.Major -lt 5) {
        throw "PowerShell $($PSVersionTable.PSVersion) is too old; need 5.1 or later."
    }
    if ($env:OS -ne 'Windows_NT') {
        throw "this installer is for Windows. On macOS or Linux use install.sh: curl -fsSL https://github.com/$repo/releases/latest/download/install.sh | sh"
    }
    if ($releaseBase -notlike 'https://*') { throw "release base URL must be https: $releaseBase" }

    # pkm installs per user: an elevated session would put it in the administrator's profile.
    if ($env:PKM_ALLOW_ROOT -ne '1') {
        $principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
        if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            throw 'this is an elevated (Administrator) session. Open a normal PowerShell window and run the command again, or set PKM_ALLOW_ROOT=1.'
        }
    }

    if (-not $InstallDir) {
        if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is not set; pass -InstallDir or set PKM_INSTALL_DIR.' }
        $InstallDir = Join-Path $env:LOCALAPPDATA 'pkm\bin'
    }
    if (-not [IO.Path]::IsPathRooted($InstallDir)) { throw "install dir must be an absolute path: $InstallDir" }
    $InstallDir = [IO.Path]::GetFullPath($InstallDir).TrimEnd('\', '/')
    $exePath = Join-Path $InstallDir $exeName
    $markerPath = Join-Path $InstallDir $markerName

    function Write-Step([string]$Text) { Write-Host $Text }

    function Get-NormalPath([string]$Path) {
        if (-not $Path) { return '' }
        return [Environment]::ExpandEnvironmentVariables($Path.Trim().Trim('"')).TrimEnd('\', '/').ToLowerInvariant()
    }

    function Test-Managed {
        if (-not (Test-Path -LiteralPath $markerPath -PathType Leaf)) { return $false }
        $first = (Get-Content -LiteralPath $markerPath -TotalCount 1)
        return ($first -eq 'pkm-managed-v1')
    }

    function Get-InstalledVersion {
        if (-not (Test-Managed)) { return '' }
        foreach ($line in (Get-Content -LiteralPath $markerPath)) {
            if ($line -like 'version=*') { return $line.Substring(8) }
        }
        return ''
    }

    # "pkm 0.1.0 (spec ...)" for v0.1.0. Returns $null when the binary does not start.
    function Get-VersionLine([string]$Exe) {
        try {
            $out = & $Exe --version 2>$null
            if ($LASTEXITCODE -ne 0) { return $null }
            return ([string]($out | Select-Object -First 1)).Trim()
        } catch {
            return $null
        }
    }
    function Test-RunsAs([string]$Exe, [string]$Ver) {
        $line = Get-VersionLine $Exe
        return ($null -ne $line -and $line.StartsWith("pkm $($Ver.TrimStart('v')) "))
    }

    function Send-EnvironmentChange {
        try {
            if (-not ('PkmInstaller.NativeMethods' -as [type])) {
                Add-Type -Namespace PkmInstaller -Name NativeMethods -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true, CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
public static extern System.IntPtr SendMessageTimeout(System.IntPtr hWnd, uint msg, System.UIntPtr wParam, string lParam, uint flags, uint timeout, out System.UIntPtr result);
'@
            }
            $result = [UIntPtr]::Zero
            # HWND_BROADCAST, WM_SETTINGCHANGE, SMTO_ABORTIFHUNG, 5 s
            [PkmInstaller.NativeMethods]::SendMessageTimeout([IntPtr]0xffff, 0x1a, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result) | Out-Null
        } catch {
            # Best effort: new terminals still read the registry.
        }
    }

    # Add ($Add = $true) or remove the install dir in HKCU\Environment\Path, keeping the value kind
    # (REG_EXPAND_SZ stays REG_EXPAND_SZ) and every other entry untouched.
    function Update-UserPath([bool]$Add) {
        $want = Get-NormalPath $InstallDir
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
        if ($null -eq $key) {
            if (-not $Add) { return $false }
            $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
        }
        try {
            $raw = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
            if ($null -ne $key.GetValue('Path')) {
                $k = $key.GetValueKind('Path')
                if ($k -eq [Microsoft.Win32.RegistryValueKind]::String) { $kind = $k }
            }
            $entries = @($raw.Split(';') | Where-Object { $_ -ne '' })
            $present = @($entries | Where-Object { (Get-NormalPath $_) -eq $want }).Count -gt 0
            if ($Add -and -not $present) {
                $key.SetValue('Path', (@($entries) + $InstallDir) -join ';', $kind)
            } elseif (-not $Add -and $present) {
                $key.SetValue('Path', (@($entries | Where-Object { (Get-NormalPath $_) -ne $want }) -join ';'), $kind)
            } else {
                return $false
            }
        } finally {
            $key.Close()
        }
        Send-EnvironmentChange
        return $true
    }

    function Test-InSessionPath {
        $want = Get-NormalPath $InstallDir
        foreach ($p in ($env:Path -split ';')) { if ((Get-NormalPath $p) -eq $want) { return $true } }
        return $false
    }

    function Remove-StaleExe {
        $left = @()
        foreach ($f in @(Get-ChildItem -LiteralPath $InstallDir -Filter "$exeName.old-*" -Force -ErrorAction SilentlyContinue)) {
            try { Remove-Item -LiteralPath $f.FullName -Force } catch { $left += $f.FullName }
        }
        return $left
    }

    function Confirm-Action([string]$Question) {
        if ($Yes) { return }
        $interactive = [Environment]::UserInteractive -and -not [Console]::IsInputRedirected
        if (-not $interactive) { throw 'confirmation needed but there is no interactive console; re-run with -Yes or PKM_YES=1.' }
        $ans = Read-Host "$Question [y/N]"
        if ($ans -notmatch '^(y|yes)$') { throw 'cancelled' }
    }

    # ---- uninstall -----------------------------------------------------------------------------
    if ($Uninstall) {
        if (-not (Test-Managed)) {
            throw "no install made by this script in $InstallDir (use -InstallDir if you installed elsewhere)"
        }
        Write-Step "will remove: $exePath and its PATH entry"
        Write-Step "will keep:   index and data in $(Join-Path $env:LOCALAPPDATA 'pkm') (delete that folder yourself to remove them)"
        if (Test-Path -LiteralPath $exePath -PathType Leaf) {
            Write-Step "will keep:   the 'pkm' entry (and an old 'kg' entry written by pkm) in Claude Desktop. To remove them first, run:"
            Write-Step "               & `"$exePath`" mcp install --remove"
        }
        if ($DryRun) { Write-Step 'dry-run: nothing removed'; return }
        Confirm-Action 'Remove pkm?'
        if (Test-Path -LiteralPath $exePath -PathType Leaf) {
            # A running pkm.exe cannot be deleted but can be renamed; the leftover goes on a later run.
            $old = Join-Path $InstallDir ("$exeName.old-$PID-" + [DateTime]::UtcNow.Ticks)
            Rename-Item -LiteralPath $exePath -NewName (Split-Path -Leaf $old)
            try { Remove-Item -LiteralPath $old -Force } catch {
                Write-Step "note: $old is still in use (Claude Desktop?). Quit Claude Desktop, then delete it."
            }
        }
        [void](Remove-StaleExe)
        Remove-Item -LiteralPath $markerPath -Force
        if (Update-UserPath $false) { Write-Step "PATH: removed $InstallDir from the user PATH" }
        $env:Path = (@($env:Path -split ';' | Where-Object { $_ -and ((Get-NormalPath $_) -ne (Get-NormalPath $InstallDir)) }) -join ';')
        if (-not @(Get-ChildItem -LiteralPath $InstallDir -Force)) { Remove-Item -LiteralPath $InstallDir -Force }
        Write-Step 'removed pkm'
        return
    }

    # ---- install / update -----------------------------------------------------------------------
    if ($Version -eq '@PKM_' + 'VERSION@') {
        throw "this copy of the installer has no version (it was not downloaded from a release). Pass -Version vX.Y.Z, or use the install.ps1 attached to a release of $repo"
    }
    if ($Version -match '^\d') { $Version = "v$Version" }
    if ($Version -notmatch '^v\d+\.\d+\.\d+([-+][0-9A-Za-z.+-]+)?$') { throw "invalid version: $Version (expected vX.Y.Z)" }
    if ($LocalFixture -and -not (Test-Path -LiteralPath $LocalFixture -PathType Container)) {
        throw "-LocalFixture is not a directory: $LocalFixture"
    }

    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    if ($arch -eq 'ARM64') {
        Write-Step 'note: Windows on ARM; installing the x64 build, which Windows 11 runs through emulation.'
    } elseif ($arch -ne 'AMD64') {
        throw "unsupported CPU: $arch (prebuilt pkm is x64 only). Build from source: cargo install --git https://github.com/$repo --locked pkm"
    }

    if ((Test-Path -LiteralPath $exePath -PathType Leaf) -and -not (Test-Managed)) {
        throw "$exePath exists and was not installed by this script; remove it or pass -InstallDir <other folder>"
    }

    $upToDate = ((Get-InstalledVersion) -eq $Version) -and (Test-Path -LiteralPath $exePath -PathType Leaf) -and (Test-RunsAs $exePath $Version)
    if ($upToDate) {
        Write-Step "pkm $Version is already installed in $InstallDir; nothing to do"
    } else {
        # Older Windows PowerShell offers TLS 1.0/1.1 by default, which GitHub refuses.
        try { [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12 } catch { }

        $tmp = Join-Path ([IO.Path]::GetTempPath()) ('pkm-install-' + [Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $tmp | Out-Null
        $state = @{ ViaGh = $false }
        try {
            # Runs gh with stderr shown but not turned into errors (PS 5.1 + 'Stop' would abort). Returns the exit code.
            function Invoke-Gh([string[]]$GhArgs, [bool]$Quiet) {
                $ErrorActionPreference = 'Continue'
                if ($Quiet) { & gh @GhArgs *> $null } else { & gh @GhArgs }
                return $LASTEXITCODE
            }

            # HTTP status of a failed Invoke-WebRequest (PS 5.1 WebException or PS 7 HttpResponseException), or 0.
            function Get-HttpStatus($ErrorRecord) {
                $prop = $ErrorRecord.Exception.PSObject.Properties['Response']
                if ($null -eq $prop -or $null -eq $prop.Value) { return 0 }
                try { return [int]$prop.Value.StatusCode } catch { return 0 }
            }

            function Get-Asset([string]$Name) {
                $dest = Join-Path $tmp $Name
                if ($LocalFixture) {
                    $src = Join-Path $LocalFixture $Name
                    if (-not (Test-Path -LiteralPath $src -PathType Leaf)) { throw "local fixture has no ${Name}: $LocalFixture" }
                    Copy-Item -LiteralPath $src -Destination $dest
                } else {
                    if (-not $state.ViaGh) {
                        $url = "$releaseBase/download/$Version/$Name"
                        try {
                            Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing -TimeoutSec 300 | Out-Null
                        } catch {
                            $status = Get-HttpStatus $_
                            if (@(401, 403, 404) -notcontains $status) {
                                throw "download failed: $url ($($_.Exception.Message)). Check your connection and retry."
                            }
                            Write-Step "note: anonymous download of $Name was refused (HTTP $status, private repository?); trying the GitHub CLI"
                            $state.ViaGh = $true
                        }
                    }
                    if ($state.ViaGh) {
                        if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
                            throw "cannot download $Name anonymously: the repository $repo is private (or the asset is missing), and the GitHub CLI 'gh' is not installed. Install it (winget install GitHub.cli), run 'gh auth login', then run this installer again."
                        }
                        if ((Invoke-Gh @('auth', 'status') $true) -ne 0) {
                            throw "cannot download $Name anonymously: the repository $repo is private, and gh is not logged in. Run 'gh auth login', then run this installer again."
                        }
                        Remove-Item -LiteralPath $dest -Force -ErrorAction SilentlyContinue
                        $code = Invoke-Gh @('release', 'download', $Version, '--repo', $repo, '--pattern', $Name, '--output', $dest, '--clobber') $false
                        if ($code -ne 0 -or -not (Test-Path -LiteralPath $dest -PathType Leaf)) {
                            throw "gh release download $Version --repo $repo --pattern $Name failed: no access to $repo, or release $Version has no $Name. Check with: gh release view $Version --repo $repo"
                        }
                    }
                }
                if ((Get-Item -LiteralPath $dest).Length -gt $maxBytes) { throw "$Name is larger than allowed" }
                return $dest
            }

            $exeTmp = Get-Asset $asset
            $sums = Get-Asset 'checksums.txt'

            # Exact file-name match, exactly one entry; a leading binary-mode '*' is ignored.
            $found = @()
            foreach ($line in [IO.File]::ReadAllLines($sums)) {
                $fields = $line.Trim() -split '\s+'
                if ($fields.Count -lt 2) { continue }
                $name = $fields[1]
                if ($name.StartsWith('*')) { $name = $name.Substring(1) }
                if ($name -ceq $asset) { $found += $fields[0] }
            }
            if ($found.Count -ne 1) { throw "checksum entry for $asset missing or duplicated in checksums.txt; the release is incomplete" }
            if ($found[0] -notmatch '^[0-9A-Fa-f]{64}$') { throw "invalid checksum entry for $asset" }
            $actual = (Get-FileHash -LiteralPath $exeTmp -Algorithm SHA256).Hash
            if ($actual -ine $found[0]) { throw "checksum mismatch for ${asset}: download corrupted or tampered. Retry; report it if it persists." }
            Write-Step "checksum ok: $asset"
            Unblock-File -LiteralPath $exeTmp -ErrorAction SilentlyContinue

            $line = Get-VersionLine $exeTmp
            if ($null -eq $line) {
                throw "the downloaded pkm.exe does not start on this machine (Windows $([Environment]::OSVersion.Version), $arch). Please report it."
            }
            if (-not (Test-RunsAs $exeTmp $Version)) { throw "the downloaded binary reports '$line', expected pkm $Version" }

            if ($DryRun) {
                Write-Step "dry-run: would install $asset $Version to $exePath"
            } else {
                New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
                [void](Remove-StaleExe)
                $newPath = Join-Path $InstallDir ".pkm.new.$PID.exe"
                Copy-Item -LiteralPath $exeTmp -Destination $newPath -Force
                $old = $null
                if (Test-Path -LiteralPath $exePath -PathType Leaf) {
                    # Renaming works even while Claude Desktop runs pkm.exe; overwriting does not.
                    $old = Join-Path $InstallDir ("$exeName.old-$PID-" + [DateTime]::UtcNow.Ticks)
                    Rename-Item -LiteralPath $exePath -NewName (Split-Path -Leaf $old)
                }
                Move-Item -LiteralPath $newPath -Destination $exePath -Force
                if (-not (Test-RunsAs $exePath $Version)) {
                    Remove-Item -LiteralPath $exePath -Force -ErrorAction SilentlyContinue
                    if ($old) { Rename-Item -LiteralPath $old -NewName $exeName }
                    throw 'the new pkm.exe failed to start; the previous version was restored'
                }
                Set-Content -LiteralPath $markerPath -Value @('pkm-managed-v1', "version=$Version") -Encoding Ascii
                if ($old) {
                    try { Remove-Item -LiteralPath $old -Force } catch {
                        Write-Step 'note: the previous pkm.exe is still running (Claude Desktop?). Restart Claude Desktop to use the new version.'
                    }
                }
                Write-Step "installed: $(Get-VersionLine $exePath) -> $exePath"
            }
        } finally {
            Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
        }
    }

    # ---- PATH ----------------------------------------------------------------------------------
    if (-not $NoModifyPath) {
        if ($DryRun) {
            Write-Step "dry-run: would make sure $InstallDir is on the user PATH"
        } elseif (Update-UserPath $true) {
            Write-Step "PATH: added $InstallDir to the user PATH (new terminals pick it up)"
        }
        if (-not $DryRun -and -not (Test-InSessionPath)) { $env:Path = "$env:Path;$InstallDir" }
    } elseif (-not (Test-InSessionPath)) {
        Write-Step "PATH: $InstallDir is not on PATH. To add it for your user:"
        Write-Step "  [Environment]::SetEnvironmentVariable('Path', [Environment]::GetEnvironmentVariable('Path', 'User') + ';$InstallDir', 'User')"
    }

    # ---- optional: pkm mcp install ---------------------------------------------------------------
    if ($McpInstall) {
        if ($DryRun) {
            Write-Step "dry-run: would run: & `"$exePath`" mcp install `"$McpInstall`""
        } else {
            if (-not (Test-Path -LiteralPath $McpInstall -PathType Container)) {
                throw "vault folder not found: $McpInstall (pkm is installed; pass the folder that holds your Obsidian vault)"
            }
            Write-Step "running: & `"$exePath`" mcp install `"$McpInstall`""
            & $exePath mcp install $McpInstall
            if ($LASTEXITCODE -ne 0) {
                throw "pkm is installed, but 'pkm mcp install' failed (see above). Fix the cause, then run: & `"$exePath`" mcp install `"$McpInstall`""
            }
        }
    }

    if (-not $DryRun) {
        Write-Host ''
        if ($McpInstall) {
            Write-Host 'Next: start Claude Desktop and ask it about your vault. If something is wrong, run:'
            Write-Host "  & `"$exePath`" doctor"
        } else {
            Write-Host 'Next: quit Claude Desktop completely (also from the tray), then run:'
            Write-Host "  & `"$exePath`" mcp install `"C:\path\to\your\vault`""
            Write-Host "  & `"$exePath`" doctor"
        }
    }
}

try {
    Invoke-PkmInstaller -Version $Version -InstallDir $InstallDir -McpInstall $McpInstall -LocalFixture $LocalFixture `
        -Repo $Repo -ReleaseBaseUrl $ReleaseBaseUrl -Uninstall $Uninstall.IsPresent -NoModifyPath $NoModifyPath.IsPresent -DryRun $DryRun.IsPresent -Yes $Yes.IsPresent
    $global:LASTEXITCODE = 0
} catch {
    [Console]::Error.WriteLine("error: $($_.Exception.Message)")
    # Run as a file, the exit code reports the failure. Piped into iex there is no file, and
    # 'exit' would close the user's window, so only the error is shown.
    if ($PSCommandPath) { exit 1 } else { $global:LASTEXITCODE = 1 }
}
