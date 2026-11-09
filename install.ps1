# Hersir installer for Windows. Builds Hersir from source and installs it for you alone (no administrator rights):
#   %LOCALAPPDATA%\Programs\Hersir\  hersir.exe, hersir-mcp.exe (the helper chat uses) and hersir-installed.txt
#   Start menu\Programs\Hersir.lnk    the shortcut that starts Hersir and lets Windows show its notifications
# An install from before 0.17.0, %LOCALAPPDATA%\Programs\Gizai with gizai.exe, is updated where it is: it gets the
# hersir names beside its gizai ones, which keep working.
# Your data lives in %APPDATA%\Gizai and is never touched by install or uninstall (unless -Purge).
# Hersir runs the coding CLIs installed on Windows itself; it never uses WSL.
#
# Usage, in PowerShell (Windows PowerShell 5.1 or PowerShell 7):
#   .\install.ps1                from a checkout: check, build and install
#   .\install.ps1 -Check         only report what is missing
#   .\install.ps1 -BuildOnly     check and build, install nothing (Hersir's own Update runs this, then -SkipBuild)
#   .\install.ps1 -SkipBuild     install the programs already built in target\release
#   .\install.ps1 -Uninstall     remove Hersir, also an install from before 0.17.0 (add -Purge to delete your data too)
# When PowerShell won't run scripts: powershell -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
# Run outside a checkout, it clones (or updates) the source into %LOCALAPPDATA%\hersir-src first.
# Environment: HERSIR_REPO (the git URL to clone), HERSIR_BRANCH (default production: the released code; main is
# development), HERSIR_PREFIX (default %LOCALAPPDATA%\Programs\Hersir), GIZAI_DATA_DIR (default %APPDATA%\Gizai). Their
# names from before 0.17.0, GIZAI_REPO, GIZAI_BRANCH and GIZAI_PREFIX, still work; a HERSIR_ one wins.
[CmdletBinding()]
param(
  [switch]$Check,
  [switch]$BuildOnly,
  [switch]$SkipBuild,
  [switch]$Uninstall,
  [switch]$Purge,
  [switch]$Help
)
Set-StrictMode -Version 2
$ErrorActionPreference = 'Stop'

function Say([string]$Text) { Write-Output $Text }
function Step([string]$Text) { Write-Output ''; Write-Output "== $Text" }
function Have([string]$Name) { [bool](Get-Command $Name -ErrorAction SilentlyContinue) }
# A path made absolute from PowerShell's own folder (.NET would resolve a relative one from another folder).
function Full([string]$Path) { $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path) }

# Runs a step's program, its output (errors included) going out as plain lines: Windows PowerShell would turn each line
# a program writes to its error output into a PowerShell error. Stops the install when the program fails.
function Invoke-Step([string]$Exe, [string[]]$Arguments) {
  $ErrorActionPreference = 'Continue'
  & $Exe @Arguments 2>&1 | ForEach-Object { "$_" }
  if ($LASTEXITCODE -ne 0) {
    Say "$Exe $($Arguments -join ' ') failed (exit code $LASTEXITCODE)."
    exit 1
  }
}

# A program's output, errors included, as one text.
function Read-Program([string]$Exe, [string[]]$Arguments) {
  $ErrorActionPreference = 'Continue'
  (& $Exe @Arguments 2>&1 | ForEach-Object { "$_" }) -join "`n"
}

# Runs hersir.exe and says whether it worked, with what it printed. hersir.exe is a window app without a console of its
# own: run directly, PowerShell neither waits for it nor shows its output. Start-Process waits, with the output in files.
function Invoke-Hersir([string]$Exe, [string[]]$Arguments) {
  $out = [System.IO.Path]::GetTempFileName()
  $err = [System.IO.Path]::GetTempFileName()
  try {
    $p = Start-Process -FilePath $Exe -ArgumentList $Arguments -NoNewWindow -Wait -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
    $text = (@(Get-Content -LiteralPath $out) + @(Get-Content -LiteralPath $err)) -join "`n"
    [pscustomobject]@{ Ok = ($p.ExitCode -eq 0); Text = $text.Trim() }
  } catch {
    [pscustomobject]@{ Ok = $false; Text = $_.Exception.Message }
  } finally {
    Remove-Item -LiteralPath $out, $err -Force -ErrorAction SilentlyContinue
  }
}

if ($Help) {
  Get-Content -LiteralPath $PSCommandPath -TotalCount 19 | ForEach-Object { $_ -replace '^# ?', '' }
  exit 0
}
if ($BuildOnly -and $SkipBuild) {
  [Console]::Error.WriteLine("-BuildOnly and -SkipBuild don't go together (see -Help)")
  exit 2
}
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
  Say 'install.ps1 is for Windows. On Linux and macOS, install Hersir with install.sh.'
  exit 1
}

$DefaultRepo = 'https://github.com/Yotech-AI/hersir.git'
# A setting from the environment: HERSIR_<name>, else GIZAI_<name>, its name before 0.17.0, which still works alone.
function Get-Setting([string]$Name) {
  foreach ($var in @("HERSIR_$Name", "GIZAI_$Name")) {
    $value = [Environment]::GetEnvironmentVariable($var)
    if ($value) { return $value }
  }
  return ''
}
# An install from before 0.17.0 (GA-248) in $Dir: gizai.exe beside gizai-installed.txt. It is updated where it is, as
# 0.16.0's own update checks and restarts <prefix>\gizai.exe: it gets the hersir names beside its gizai ones.
function Test-OldInstall([string]$Dir) {
  (Test-Path -LiteralPath (Join-Path $Dir 'gizai.exe')) -and (Test-Path -LiteralPath (Join-Path $Dir 'gizai-installed.txt'))
}
$Repo = Get-Setting 'REPO'
if (-not $Repo) { $Repo = $DefaultRepo }
$NewPrefix = Full (Join-Path $env:LOCALAPPDATA 'Programs\Hersir')
$OldPrefix = Full (Join-Path $env:LOCALAPPDATA 'Programs\Gizai')
$GivenPrefix = Get-Setting 'PREFIX'
# Without one given: the usual folder of an install from before 0.17.0 when there is one, else Hersir's.
$Prefix = if ($GivenPrefix) { Full $GivenPrefix } elseif (Test-OldInstall $OldPrefix) { $OldPrefix } else { $NewPrefix }
$OldLayout = Test-OldInstall $Prefix
$Data = if ($env:GIZAI_DATA_DIR) { $env:GIZAI_DATA_DIR } else { Join-Path $env:APPDATA 'Gizai' }
$Data = Full $Data
$SrcClone = Join-Path $env:LOCALAPPDATA 'hersir-src'
# The source copy from before 0.17.0: an install removes it, unless it runs from it.
$OldSrcClone = Join-Path $env:LOCALAPPDATA 'gizai-src'
$StartMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$Shortcut = Join-Path $StartMenu 'Hersir.lnk'
# The shortcut's name before the app was called Hersir (GA-233): an install or uninstall removes it.
$OldShortcut = Join-Path $StartMenu 'Gizai.lnk'
$Exe = Join-Path $Prefix 'hersir.exe'
# An installed Hersir knows its install by this file (a build in target\release has none). One from before 0.17.0 keeps
# its own, gizai-installed.txt, too.
$Marker = Join-Path $Prefix 'hersir-installed.txt'
$OldMarker = Join-Path $Prefix 'gizai-installed.txt'
# What the build makes, and the names it goes in under (name = what the build made). An install from before 0.17.0 gets
# gizai.exe and gizai-mcp.exe too, as copies (a link may need rights you don't have): the gizai command and 0.16.0's own
# update start the new Hersir.
$BuiltNames = @('hersir.exe', 'hersir-mcp.exe')
$Programs = [ordered]@{ 'hersir.exe' = 'hersir.exe'; 'hersir-mcp.exe' = 'hersir-mcp.exe' }
if ($OldLayout) { $Programs['gizai.exe'] = 'hersir.exe'; $Programs['gizai-mcp.exe'] = 'hersir-mcp.exe' }

# Removes the Start menu shortcut $Link when it starts the Hersir installed in one of $Dirs: hersir.exe, or gizai.exe
# (before 0.17.0).
function Remove-OwnShortcut([string]$Link, [string[]]$Dirs) {
  if (Test-Path -LiteralPath $Link) {
    $target = (New-Object -ComObject WScript.Shell).CreateShortcut($Link).TargetPath
    $own = foreach ($dir in $Dirs) { Join-Path $dir 'hersir.exe'; Join-Path $dir 'gizai.exe' }
    if (@($own) -contains $target) { Remove-Item -LiteralPath $Link -Force }
  }
}

# ---------- uninstall ----------
if ($Uninstall) {
  Step 'Removing Hersir'
  # Both layouts: Hersir's names, and the gizai ones of an install from before 0.17.0. Without a prefix given, in both
  # usual folders.
  $dirs = if ($GivenPrefix) { @($Prefix) } else { @($NewPrefix, $OldPrefix) }
  $files = @('hersir.exe', 'hersir-mcp.exe', 'hersir-installed.txt', 'gizai.exe', 'gizai-mcp.exe', 'gizai-installed.txt')
  try {
    foreach ($dir in $dirs) {
      foreach ($f in $files) {
        $path = Join-Path $dir $f
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
      }
    }
  } catch {
    Say "Could not remove Hersir from ${dir}: is it open? Quit it (Quit Hersir completely, in its tray menu), then run this again."
    exit 1
  }
  foreach ($dir in $dirs) {
    Get-ChildItem -LiteralPath $dir -Force -Filter '.*.old.*' -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue
  }
  # The Start menu shortcuts go only when they start this install's Hersir: Hersir.lnk, and Gizai.lnk from before GA-233.
  Remove-OwnShortcut $Shortcut $dirs
  Remove-OwnShortcut $OldShortcut $dirs
  foreach ($dir in $dirs) {
    if (-not (Get-ChildItem -LiteralPath $dir -Force -ErrorAction SilentlyContinue)) {
      Remove-Item -LiteralPath $dir -Force -ErrorAction SilentlyContinue
    }
  }
  Say 'Removed the app and the Start menu shortcut.'
  if ($Purge) {
    Remove-Item -LiteralPath $Data, $SrcClone, $OldSrcClone -Recurse -Force -ErrorAction SilentlyContinue
    Say "Deleted your data ($Data) and the source copy ($SrcClone)."
  } else {
    Say "Your data stays in $Data (run with -Uninstall -Purge to delete it)."
  }
  exit 0
}

# ---------- check ----------
Step 'Checking what Hersir needs'
$notes = New-Object System.Collections.Generic.List[string]
if (-not (Have 'git')) {
  $notes.Add('Git for Windows is missing (Claude Code needs it too): winget install --id Git.Git -e   (then open a new terminal)')
}
if (-not $SkipBuild) {
  if (Have 'cargo') {
    $rustc = Read-Program 'rustc' @('-vV')
    if ($rustc -match 'host: (\S+)' -and $Matches[1] -notlike '*-windows-msvc') {
      $notes.Add("Rust builds for $($Matches[1]) here; Hersir needs Rust's MSVC toolchain: rustup default stable-msvc")
    }
  } else {
    $notes.Add('Rust is missing: winget install --id Rustlang.Rustup -e, or rustup-init.exe from https://rustup.rs   (then open a new terminal)')
  }
  $nodeFrom = 'winget install --id OpenJS.NodeJS.LTS -e, or from https://nodejs.org'
  if (Have 'node') {
    $nodeVersion = Read-Program 'node' @('-v')
    if (-not ($nodeVersion -match '^v(\d+)\.' -and [int]$Matches[1] -ge 20)) {
      $notes.Add("Node.js 20 or newer is needed (found $nodeVersion): $nodeFrom")
    }
  } else {
    $notes.Add("Node.js 20 or newer is missing: $nodeFrom")
  }
  if (-not (Have 'npm.cmd')) { $notes.Add('npm is missing (it comes with Node.js)') }
  # Rust's MSVC toolchain links with the Microsoft C++ Build Tools (Visual Studio's "Desktop development with C++").
  $vc = ''
  if (${env:ProgramFiles(x86)}) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vswhere) {
      $vc = Read-Program $vswhere @('-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
        'Microsoft.VisualStudio.Component.VC.Tools.ARM64', '-requiresAny', '-property', 'installationPath')
    }
  }
  if (-not $vc.Trim()) {
    $notes.Add('The Microsoft C++ Build Tools are missing: install them with "Desktop development with C++" from https://visualstudio.microsoft.com/visual-cpp-build-tools/ (rustup-init offers to as well)')
  }
}
$ok = $notes.Count -eq 0
foreach ($n in $notes) { Say $n }
$claude = Get-Command 'claude' -ErrorAction SilentlyContinue | Select-Object -First 1
if ($claude) {
  Say "Claude Code: found ($($claude.Source)). Make sure you are logged in: run 'claude' once."
} else {
  Say "Claude Code: not found. Hersir's agents and chat run it; install it from https://docs.claude.com/en/docs/claude-code and log in."
}
# Hersir's window is WebView2, which comes with Windows 11. Building doesn't need it; starting Hersir does.
$webView = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
$hasWebView = $false
foreach ($key in @("HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$webView", "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$webView",
    "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$webView")) {
  $pv = Get-ItemProperty -LiteralPath $key -Name 'pv' -ErrorAction SilentlyContinue
  if ($pv -and $pv.pv -and $pv.pv -ne '0.0.0.0') { $hasWebView = $true }
}
if (-not $hasWebView) {
  Say "WebView2: not found, so Hersir's window can't open. Windows 11 comes with it; get the Evergreen Runtime from https://developer.microsoft.com/microsoft-edge/webview2/"
}
if ($ok) { Say 'Everything Hersir needs to build is here.' }
if ($Check) { if ($ok) { exit 0 } else { exit 1 } }
if (-not $ok -and -not $SkipBuild) {
  Say ''
  Say 'Install the missing pieces above, then run this again.'
  exit 1
}

# ---------- source ----------
$conf = if ($PSScriptRoot) { Join-Path $PSScriptRoot 'src-tauri\tauri.conf.json' } else { '' }
if ($conf -and (Test-Path -LiteralPath $conf) -and (Select-String -LiteralPath $conf -Pattern '"productName": "Hersir"' -SimpleMatch -Quiet)) {
  $Src = $PSScriptRoot
} else {
  Step 'Getting the source'
  if (Test-Path -LiteralPath (Join-Path $SrcClone '.git')) {
    Invoke-Step 'git' @('-C', $SrcClone, 'pull', '--ff-only')
  } else {
    $branch = Get-Setting 'BRANCH'
    if (-not $branch) { $branch = 'production' }
    Invoke-Step 'git' @('clone', '--depth', '1', '--branch', $branch, $Repo, $SrcClone)
  }
  $Src = $SrcClone
}
Say "Source: $Src"

# ---------- build ----------
$Release = Join-Path $Src 'target\release'
if (-not $SkipBuild) {
  Step 'Building (this takes a few minutes the first time)'
  $quiet = @{ TAURI_TELEMETRY_DISABLED = '1'; npm_config_update_notifier = 'false'; npm_config_fund = 'false'; npm_config_audit = 'false' }
  $before = @{}
  foreach ($name in @($quiet.Keys)) {
    $before[$name] = [Environment]::GetEnvironmentVariable($name)
    [Environment]::SetEnvironmentVariable($name, $quiet[$name])
  }
  Push-Location -LiteralPath $Src
  try {
    # npm.cmd rather than npm: no npm.ps1, which the execution policy may refuse.
    Invoke-Step 'npm.cmd' @('ci')
    # The Tauri CLI, never a plain cargo build --release: that can make a hersir.exe that loads the dev URL. Called
    # directly rather than through npm run with "--", which Windows PowerShell may drop before npm sees it.
    Invoke-Step (Join-Path $Src 'node_modules\.bin\tauri.cmd') @('build', '--no-bundle')
    # the crate gizai-mcp builds the program hersir-mcp.exe
    Invoke-Step 'cargo' @('build', '--release', '-p', 'gizai-mcp')
  } finally {
    Pop-Location
    foreach ($name in @($before.Keys)) { [Environment]::SetEnvironmentVariable($name, $before[$name]) }
  }
}
foreach ($p in $BuiltNames) {
  if (-not (Test-Path -LiteralPath (Join-Path $Release $p))) {
    Say "$(Join-Path $Release $p) is missing: build first (run without -SkipBuild)."
    exit 1
  }
}
$NewHersir = Join-Path $Release 'hersir.exe'
if ($BuildOnly) {
  $built = Invoke-Hersir $NewHersir @('--version')
  $name = if ($built.Ok -and $built.Text) { $built.Text } else { 'hersir' }
  Step "Built: $name, in $Release"
  Say "Nothing was installed. Install it with: $(Join-Path $Src 'install.ps1') -SkipBuild"
  exit 0
}

# ---------- back up ----------
# Before replacing a Hersir you use, snapshot its data (the new build opens it, and may upgrade it, on its next start).
$db = Join-Path $Data 'gizai.db'
if (Test-Path -LiteralPath $db) {
  Step 'Backing up your data'
  # The data folder goes to the new Hersir under both its names (HERSIR_DATA_DIR wins over GIZAI_DATA_DIR).
  $dataBefore = $env:HERSIR_DATA_DIR; $gizaiDataBefore = $env:GIZAI_DATA_DIR
  $env:HERSIR_DATA_DIR = $Data; $env:GIZAI_DATA_DIR = $Data
  try { $snap = Invoke-Hersir $NewHersir @('--backup', 'before-install') } finally {
    $env:HERSIR_DATA_DIR = $dataBefore; $env:GIZAI_DATA_DIR = $gizaiDataBefore
  }
  if ($snap.Ok) {
    Say "Backed up your data to $($snap.Text)"
  } else {
    Say "Could not back up ${db}: $($snap.Text)"
    Say 'So nothing was installed. Move that file aside (or fix it), then run this again.'
    exit 1
  }
}

# ---------- install ----------
Step 'Installing'
New-Item -ItemType Directory -Force -Path $Prefix | Out-Null
# Programs an earlier install renamed aside go now (one that still runs stays until the next install).
Get-ChildItem -LiteralPath $Prefix -Force -Filter '.*.old.*' -ErrorAction SilentlyContinue | Remove-Item -Force -ErrorAction SilentlyContinue
# Each program goes in under a temporary name of this installer's own first, then replaces the old one with renames: a
# copy that fails (a full disk) leaves the installed Hersir as it was. Windows won't overwrite a program that runs (an
# open Hersir, or the helper a chat uses) but can rename it, so the old one is renamed aside first.
$new = @{}; $old = @{}
$names = @($Programs.Keys)
foreach ($p in $names) { $new[$p] = Join-Path $Prefix ".$p.new.$PID"; $old[$p] = Join-Path $Prefix ".$p.old.$PID" }
try {
  foreach ($p in $names) { Copy-Item -LiteralPath (Join-Path $Release $Programs[$p]) -Destination $new[$p] -Force }
} catch {
  foreach ($p in $names) { Remove-Item -LiteralPath $new[$p] -Force -ErrorAction SilentlyContinue }
  Say "Could not copy Hersir into ${Prefix}, so nothing was installed."
  exit 1
}
try {
  foreach ($p in $names) {
    $dst = Join-Path $Prefix $p
    if (Test-Path -LiteralPath $dst) { [System.IO.File]::Move($dst, $old[$p]) }
  }
  foreach ($p in $names) { [System.IO.File]::Move($new[$p], (Join-Path $Prefix $p)) }
} catch {
  $why = $_.Exception.Message
  # Put back what was renamed aside, so the Hersir you had stays as it was.
  foreach ($p in $names) {
    $dst = Join-Path $Prefix $p
    if ((Test-Path -LiteralPath $old[$p]) -and -not (Test-Path -LiteralPath $dst)) { [System.IO.File]::Move($old[$p], $dst) }
    Remove-Item -LiteralPath $new[$p] -Force -ErrorAction SilentlyContinue
  }
  Say "Could not replace Hersir in ${Prefix}: $why"
  Say 'So nothing was installed. If Hersir is open, quit it (Quit Hersir completely, in its tray menu), then run this again.'
  exit 1
}
foreach ($p in $names) { Remove-Item -LiteralPath $old[$p] -Force -ErrorAction SilentlyContinue }
$installed = Invoke-Hersir $Exe @('--version')
$version = if ($installed.Ok -and $installed.Text) { $installed.Text } else { 'hersir' }
Set-Content -LiteralPath $Marker -Value $version -Encoding Ascii
if ($OldLayout) { Set-Content -LiteralPath $OldMarker -Value $version -Encoding Ascii }

# The Start menu shortcut carries Hersir's AppUserModelID, dev.hersir.app (ai.gizai.app before 0.17.0): Windows shows a
# desktop app's notifications only for an ID that a Start menu shortcut has. WScript.Shell can't set one, so this C#
# sets it through IPropertyStore. The shortcut starts hersir.exe, also in an install from before 0.17.0.
$shortcutCode = @'
using System;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.ComTypes;

namespace HersirInstall
{
    [ComImport, Guid("00021401-0000-0000-C000-000000000046")]
    class CShellLink { }

    [ComImport, Guid("000214F9-0000-0000-C000-000000000046"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IShellLinkW
    {
        void GetPath(IntPtr pszFile, int cch, IntPtr pfd, uint fFlags);
        void GetIDList(out IntPtr ppidl);
        void SetIDList(IntPtr pidl);
        void GetDescription(IntPtr pszName, int cch);
        void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string pszName);
        void GetWorkingDirectory(IntPtr pszDir, int cch);
        void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string pszDir);
        void GetArguments(IntPtr pszArgs, int cch);
        void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string pszArgs);
        void GetHotkey(out ushort pwHotkey);
        void SetHotkey(ushort wHotkey);
        void GetShowCmd(out int piShowCmd);
        void SetShowCmd(int iShowCmd);
        void GetIconLocation(IntPtr pszIconPath, int cch, out int piIcon);
        void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)] string pszIconPath, int iIcon);
        void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)] string pszPathRel, uint dwReserved);
        void Resolve(IntPtr hwnd, uint fFlags);
        void SetPath([MarshalAs(UnmanagedType.LPWStr)] string pszFile);
    }

    [StructLayout(LayoutKind.Sequential, Pack = 4)]
    struct PropertyKey
    {
        public Guid FormatId;
        public uint PropertyId;
    }

    [StructLayout(LayoutKind.Explicit, Size = 24)]
    struct PropVariant
    {
        [FieldOffset(0)] public ushort VarType;
        [FieldOffset(8)] public IntPtr Pointer;
    }

    [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IPropertyStore
    {
        void GetCount(out uint cProps);
        void GetAt(uint iProp, out PropertyKey pkey);
        void GetValue(ref PropertyKey key, out PropVariant pv);
        void SetValue(ref PropertyKey key, ref PropVariant pv);
        void Commit();
    }

    public static class Shortcut
    {
        // Saves a shortcut at lnkPath that starts target, with the AppUserModelID appId (PKEY_AppUserModel_ID).
        public static void Save(string lnkPath, string target, string workingDir, string description, string appId)
        {
            IShellLinkW link = (IShellLinkW)new CShellLink();
            link.SetPath(target);
            link.SetWorkingDirectory(workingDir);
            link.SetDescription(description);
            link.SetIconLocation(target, 0);
            PropertyKey key = new PropertyKey();
            key.FormatId = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3");
            key.PropertyId = 5;
            PropVariant value = new PropVariant();
            value.VarType = 31; // VT_LPWSTR
            value.Pointer = Marshal.StringToCoTaskMemUni(appId);
            try
            {
                IPropertyStore store = (IPropertyStore)link;
                store.SetValue(ref key, ref value);
                store.Commit();
            }
            finally
            {
                Marshal.FreeCoTaskMem(value.Pointer);
            }
            ((IPersistFile)link).Save(lnkPath, true);
            Marshal.ReleaseComObject(link);
        }
    }
}
'@
$madeShortcut = $true
try {
  if (-not ('HersirInstall.Shortcut' -as [type])) { Add-Type -TypeDefinition $shortcutCode }
  New-Item -ItemType Directory -Force -Path $StartMenu | Out-Null
  [HersirInstall.Shortcut]::Save($Shortcut, $Exe, $Prefix, 'Clients, projects and tasks, worked on by local AI agents', 'dev.hersir.app')
} catch {
  # Without the C# (a locked-down PowerShell), a plain shortcut: Hersir starts from the Start menu, without notifications.
  $why = $_.Exception.Message
  try {
    $link = (New-Object -ComObject WScript.Shell).CreateShortcut($Shortcut)
    $link.TargetPath = $Exe
    $link.WorkingDirectory = $Prefix
    $link.Save()
    Say "Note: the Start menu shortcut has no app ID ($why), so Windows may not show Hersir's notifications."
  } catch {
    $madeShortcut = $false
    Say "Could not make the Start menu shortcut ($why). Start Hersir with: $Exe"
  }
}
# Gizai.lnk goes once Hersir.lnk is there, when it starts this install's Hersir (an install from before GA-233 made it).
if ($madeShortcut) { Remove-OwnShortcut $OldShortcut @($Prefix) }

# The one line's source copy from before 0.17.0, gizai-src (it is hersir-src now), goes, unless this runs from it.
$fromOldClone = ((Full $Src) + '\').StartsWith((Full $OldSrcClone) + '\', [StringComparison]::OrdinalIgnoreCase)
if (-not $fromOldClone -and (Test-Path -LiteralPath $OldSrcClone)) {
  Remove-Item -LiteralPath $OldSrcClone -Recurse -Force -ErrorAction SilentlyContinue
  if (Test-Path -LiteralPath $OldSrcClone) { Say "Note: could not remove $OldSrcClone, the source copy from before 0.17.0. Remove it yourself." }
}

Step "Done: $version"
Say "Start Hersir from the Start menu, or run: & '$Exe'"
if ($OldLayout) { Say "Your install from before 0.17.0 stays in $Prefix, and gizai.exe there still works." }
Say "Your data: $Data"
Say 'Update: Hersir offers new releases above Company in its sidebar (or run this installer again). Remove: run it with -Uninstall.'
