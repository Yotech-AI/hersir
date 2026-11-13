# Hersir's one-line install for Windows, in PowerShell (Windows PowerShell 5.1 or PowerShell 7):
#   irm https://hersir.dev/install.ps1 | iex
# The website sends hersir.dev/install.ps1 on to this script, scripts/get.ps1 on the production branch, on GitHub.
# It gets the source of the latest release, the tag Hersir's own update fetches (docs/RELEASING.md), into
# %LOCALAPPDATA%\hersir-src, or updates it there. Before anything of the release runs, it checks the release's
# signature, hersir-X.Y.Z.sig, with ssh-keygen (Windows' OpenSSH Client's, else Git for Windows') against Hersir's
# release keys, built in below: no signature, or one that doesn't check, and nothing is installed. Then it gets the
# release's ready-made build, checks it against the release's SHA256SUMS and unpacks it into the source, and runs that
# release's install.ps1 with Windows PowerShell: install.ps1 -Check -SkipBuild first, then install.ps1 -SkipBuild,
# which installs it. Without a ready-made build for this system, or when it can't be downloaded or doesn't match,
# install.ps1 builds the release's checked source instead, as it does from a clone. Run it again to update. Nothing
# goes into Hersir's data folder. The source folder is this script's own: changes in it are replaced. Before 0.17.0 it
# was gizai-src: that one becomes hersir-src, with its build, so the next build is quicker.
#
# Options go to install.ps1:  & ([scriptblock]::Create((irm <url>))) -Check   (see -Help)
# -FromSource is the line's own: it builds the release's checked source, as before.
# Environment: HERSIR_PREFIX goes to install.ps1 too ($env:HERSIR_PREFIX = 'D:\Apps\Hersir' first). HERSIR_REPO is the
# git repository, HERSIR_RELEASES_URL what it asks for the latest release (by default GitHub's; also http and file
# URLs), HERSIR_BUILDS_URL where it reads the release's signature, SHA256SUMS and ready-made builds (by default the
# release's own downloads on GitHub; also http and file URLs), and HERSIR_REF a branch or commit to get instead of the
# latest release, which is built from source without a signature (for tests and CI). HERSIR_RELEASE_SIGNERS, a file
# in OpenSSH's allowed-signers format, replaces the built-in keys: for tests, which sign fake releases with a throwaway
# key. Their names from before 0.17.0, GIZAI_PREFIX, GIZAI_REPO, GIZAI_RELEASES_URL, GIZAI_BUILDS_URL and GIZAI_REF,
# still work; a HERSIR_ one wins.
# Piped into iex, it runs in your own PowerShell session: its StrictMode and preferences stay its own, only its options
# ($Check and the others, all off) are left as variables, and it ends without exit, which would close the window (its
# result is in $LASTEXITCODE). Run as a file, it exits with it. A mistyped option stops it before anything happens.
# Plain ASCII only: Windows PowerShell 5.1 reads a file without a byte order mark in the ANSI code page.
[CmdletBinding()]
param(
  [switch]$Check,
  [switch]$BuildOnly,
  [switch]$SkipBuild,
  [switch]$Uninstall,
  [switch]$Purge,
  [switch]$FromSource,
  [switch]$Help
)

# Everything runs in a scope of its own. {}.File is this script's file when it runs as one, and empty from irm | iex.
& {
  param([bool]$AsFile)
  Set-StrictMode -Version 2
  $ErrorActionPreference = 'Stop'
  $ProgressPreference = 'SilentlyContinue'
  # A program that fails is no PowerShell error here (PowerShell 7.3 and later can make it one): its exit code says.
  $PSNativeCommandUseErrorActionPreference = $false

  # What this script gets comes from GitHub, so an install never depends on the website: the source from $DefaultRepo,
  # the latest release from GitHub's API, its signature and builds from the release's downloads. The one lines it only
  # shows you are hersir.dev's.
  $DefaultRepo = 'https://github.com/Yotech-AI/hersir.git'
  $GetPs1 = 'https://hersir.dev/install.ps1'
  $GetSh = 'https://hersir.dev/install.sh'
  $Namespace = 'hersir-release'
  $NoSshKeygen = "Hersir checks each release's signature with ssh-keygen: add Windows' OpenSSH Client (Settings > System > Optional features), or install Git for Windows, which has one, then run this again."

  # Hersir's release keys: the lines of release-signers, at the repository's root, exactly as they are there (a test
  # checks it). Each release's signature must check against one of them; keys never come from a release.
  $ReleaseSigners = @'
hersir-release namespaces="hersir-release" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPyS5akxwuAio54EJUMpJgss6XvxkyznQ3sxZKpOm4mX Hersir release key
hersir-release namespaces="hersir-release" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJ9ER9k9DUooCPSOMQaWoha6E4IFW+e3t08ZmH1llz3J Hersir spare release key
'@

  function Say([string]$Text) { Write-Output $Text }
  function Step([string]$Text) { Write-Output ''; Write-Output "== $Text" }
  function Have([string]$Name) { [bool](Get-Command $Name -ErrorAction SilentlyContinue) }
  # The result: as a file, the exit code; from iex only $LASTEXITCODE, as exit would close your PowerShell window.
  function Finish([int]$Code) {
    $global:LASTEXITCODE = $Code
    if ($AsFile) { exit $Code }
  }
  # Runs a program, its output (errors included) going out as plain lines: Windows PowerShell would turn each line a
  # program writes to its error output into a PowerShell error. Its exit code is in $LASTEXITCODE.
  function Invoke-Shown([string]$Exe, [string[]]$Arguments) {
    $ErrorActionPreference = 'Continue'
    & $Exe @Arguments 2>&1 | ForEach-Object { "$_" }
  }
  # A program's output, without its error output, which goes. Its exit code is in $LASTEXITCODE.
  function Read-Output([string]$Exe, [string[]]$Arguments) {
    $ErrorActionPreference = 'Continue'
    & $Exe @Arguments 2>$null
  }
  # A setting from the environment: HERSIR_<name>, else GIZAI_<name>, its name before 0.17.0, which still works alone.
  function Get-Setting([string]$Name) {
    foreach ($var in @("HERSIR_$Name", "GIZAI_$Name")) {
      $value = [Environment]::GetEnvironmentVariable($var)
      if ($value) { return $value }
    }
    return ''
  }
  # ssh-keygen.exe: Windows' own, from its OpenSSH Client, else Git for Windows' (<Git>\usr\bin), or ''.
  function Find-SshKeygen {
    foreach ($folder in @('System32', 'Sysnative')) {
      $exe = Join-Path $env:SystemRoot "$folder\OpenSSH\ssh-keygen.exe"
      if (Test-Path -LiteralPath $exe -PathType Leaf) { return $exe }
    }
    $git = Get-Command 'git.exe' -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($git) {
      # git.exe is in <Git>\cmd, <Git>\bin or <Git>\mingw64\bin
      $up = Split-Path -Parent $git.Source
      foreach ($i in 1..3) {
        $exe = Join-Path $up 'usr\bin\ssh-keygen.exe'
        if (Test-Path -LiteralPath $exe -PathType Leaf) { return $exe }
        $up = Split-Path -Parent $up
        if (-not $up) { break }
      }
    }
    return ''
  }
  # Downloads the release's file $Name from $builds into $tmp: 'ok', 'missing' when the release has no such file
  # (GitHub answers 404; a file:// one isn't there), else why it couldn't be read. Neither Invoke-WebRequest nor the
  # copy marks the file as downloaded from the internet: it gets no Zone.Identifier.
  function Get-File([string]$Name, [int]$Seconds) {
    $to = Join-Path $tmp $Name
    if ($builds -like 'file:*') {
      $from = Join-Path ([Uri]$builds).LocalPath $Name
      if (-not (Test-Path -LiteralPath $from -PathType Leaf)) { return 'missing' }
      try { Copy-Item -LiteralPath $from -Destination $to -Force; return 'ok' } catch { return $_.Exception.Message }
    }
    try {
      Invoke-WebRequest -Uri "$builds/$Name" -OutFile $to -UseBasicParsing -TimeoutSec $Seconds
      return 'ok'
    } catch {
      $why = $_.Exception.Message
      $status = 0
      try { $status = [int]$_.Exception.Response.StatusCode } catch { }
      Remove-Item -LiteralPath $to -Force -ErrorAction SilentlyContinue
      if ($status -eq 404) { return 'missing' }
      return $why
    }
  }
  # Runs ssh-keygen with the file $In as its input, byte for byte: never text piped into it, which Windows PowerShell
  # would re-encode. Its exit code, and what it wrote.
  function Invoke-SshKeygen([string[]]$Arguments, [string]$In) {
    $out = Join-Path $tmp 'ssh-keygen.out'
    $err = Join-Path $tmp 'ssh-keygen.err'
    $run = @{
      FilePath = $sshKeygen; ArgumentList = @($Arguments | ForEach-Object { '"' + $_ + '"' }); NoNewWindow = $true
      Wait = $true; PassThru = $true; RedirectStandardOutput = $out; RedirectStandardError = $err
    }
    if ($In) { $run.RedirectStandardInput = $In }
    try {
      $p = Start-Process @run
      $text = (@(Get-Content -LiteralPath $out) + @(Get-Content -LiteralPath $err)) -join "`n"
      [pscustomobject]@{ Code = $p.ExitCode; Text = $text }
    } catch {
      [pscustomobject]@{ Code = -1; Text = $_.Exception.Message }
    }
  }
  # The release's signature (docs/RELEASING.md, step 5): hersir-X.Y.Z.sig, made with one of the keys in $Signers over
  # this message, byte for byte: the line "hersir vX.Y.Z <commit>" and a newline, where <commit> is the commit of the
  # tag as fetched, followed by the release's SHA256SUMS as it is. '' when it checks, else the line that says why.
  function Test-Signature([string]$Signers) {
    $message = Join-Path $tmp 'message'
    $head = [Text.Encoding]::ASCII.GetBytes("hersir $tag $commit`n")
    [IO.File]::WriteAllBytes($message, [byte[]]($head + [IO.File]::ReadAllBytes((Join-Path $tmp 'SHA256SUMS'))))
    $signature = Join-Path $tmp $sig
    $verify = Invoke-SshKeygen @('-Y', 'verify', '-f', $Signers, '-I', $Namespace, '-n', $Namespace, '-s', $signature) $message
    if ($verify.Code -eq 0) { return '' }
    # An ssh-keygen from before OpenSSH 8.1 has no -Y.
    if ($verify.Text -match "option -- '?Y") { return $NoSshKeygen }
    # A signature by one of the keys that doesn't check was made over another message: another commit, or another
    # SHA256SUMS.
    $find = Invoke-SshKeygen @('-Y', 'find-principals', '-f', $Signers, '-s', $signature) ''
    if ($find.Code -eq 0) {
      return "The signature of $tag is by Hersir's release key, but not for its commit $commit and its SHA256SUMS: this isn't the release that was signed, so nothing was installed."
    }
    return "The signature of $tag doesn't check against Hersir's release keys, so nothing was installed."
  }
  # The release's ready-made build for this system, from its SHA256SUMS: its archive and checksum, or why there is none.
  function Find-ReadyMade {
    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    if ($arch -ne 'AMD64') {
      return @{ Why = "There is no ready-made build of Hersir for Windows on $arch, so $tag is built from source." }
    }
    $archive = "hersir-$version-windows-x86_64.zip"
    $want = ''
    foreach ($line in [IO.File]::ReadAllLines((Join-Path $tmp 'SHA256SUMS'))) {
      if ($line -cmatch '^([0-9a-f]{64})  (.+)$' -and $Matches[2] -ceq $archive) { $want = $Matches[1] }
    }
    if (-not $want) {
      return @{ Why = "$tag has no ready-made build for Windows on x86_64 ($archive isn't in its SHA256SUMS), so it is built from source." }
    }
    return @{ Why = ''; Archive = $archive; Sum = $want }
  }
  # Downloads the archive, checks it against SHA256SUMS and unpacks the two programs install.ps1 -SkipBuild installs,
  # in place of those an earlier build or download left: '' when they are in place, else the line that says why not.
  function Get-ReadyMade([string]$Archive, [string]$Sum) {
    $got = Get-File $Archive 900
    if ($got -eq 'missing') { return "$tag has no $Archive to download, so it is built from source." }
    if ($got -ne 'ok') { return "Could not download $Archive ($got), so $tag is built from source." }
    $file = Join-Path $tmp $Archive
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $file).Hash.ToLowerInvariant() -ne $Sum) {
      return "$Archive doesn't match its checksum in SHA256SUMS, so $tag is built from source."
    }
    try {
      Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
      $zip = [System.IO.Compression.ZipFile]::OpenRead($file)
      try {
        foreach ($name in @('hersir.exe', 'hersir-mcp.exe')) {
          $entry = $zip.GetEntry("target/release/$name")
          if (-not $entry) { throw "it has no target/release/$name" }
          $to = Join-Path $dir "target\release\$name"
          New-Item -ItemType Directory -Force -Path (Split-Path -Parent $to) | Out-Null
          [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $to, $true)
        }
      } finally {
        $zip.Dispose()
      }
    } catch {
      return "Could not unpack $Archive ($($_.Exception.Message)), so $tag is built from source."
    }
    return ''
  }

  if ($Help) {
    Say "Hersir's one-line install for Windows, in PowerShell:"
    Say "  irm $GetPs1 | iex"
    Say 'It gets the source of the latest release into %LOCALAPPDATA%\hersir-src (or updates it there) and checks the'
    Say "release's signature against Hersir's release keys. Then it installs the release's ready-made build, with that"
    Say "release's install.ps1: -Check first, then the install. Without one, it builds the release's source. Run it again"
    Say 'to update.'
    Say ''
    Say 'Options go to install.ps1:'
    Say '  & ([scriptblock]::Create((irm <url>))) -Check        check the release and show what is missing'
    Say '  & ([scriptblock]::Create((irm <url>))) -Uninstall    remove Hersir; add -Purge to delete your data and the source'
    Say "  & ([scriptblock]::Create((irm <url>))) -FromSource   build the release's source instead of its ready-made build"
    Say '  -BuildOnly and -SkipBuild work as they do for install.ps1 (see install.ps1 -Help).'
    Say 'Environment: HERSIR_PREFIX (default %LOCALAPPDATA%\Programs\Hersir), as for install.ps1; HERSIR_REPO,'
    Say "HERSIR_RELEASES_URL, HERSIR_BUILDS_URL (where the release's signature, SHA256SUMS and builds are), HERSIR_REF (a"
    Say 'branch or commit to build instead of the latest release, without a signature, for tests) and'
    Say "HERSIR_RELEASE_SIGNERS (an allowed-signers file instead of Hersir's release keys, for tests). Their GIZAI_ names"
    Say 'from before 0.17.0 still work.'
    Finish 0; return
  }

  # install.ps1 -Check -Uninstall would uninstall
  if ($Check -and $Uninstall) {
    Say "-Check and -Uninstall don't go together (see -Help)"
    Finish 2; return
  }
  if ($FromSource -and $SkipBuild) {
    Say "-FromSource and -SkipBuild don't go together (see -Help)"
    Finish 2; return
  }

  $tmp = ''
  try {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
      Say "This installs Hersir on Windows. On Linux and macOS, run: curl -fsSL $GetSh | sh"
      Finish 1; return
    }
    if (-not (Have 'git')) {
      Say 'To install Hersir, first install Git for Windows (winget install --id Git.Git -e), then open a new PowerShell window and run this again.'
      Finish 1; return
    }

    $repo = Get-Setting 'REPO'
    if (-not $repo) { $repo = $DefaultRepo }
    # a folder of the repository: as a full path, as git runs in the source folder
    if (Test-Path -LiteralPath $repo -PathType Container) { $repo = (Resolve-Path -LiteralPath $repo).ProviderPath }
    $github = ''
    if ($repo -match '^(https://github\.com/|git@github\.com:|ssh://git@github\.com/)(.+?)(\.git)?/?$') { $github = $Matches[2] }
    $ref = Get-Setting 'REF'
    $tag = ''
    # A release is installed only once its signature checks, with ssh-keygen. HERSIR_REF gets no release.
    if (-not $ref) {
      $sshKeygen = Find-SshKeygen
      if (-not $sshKeygen) { Say $NoSshKeygen; Finish 1; return }
    }

    # GitHub takes TLS 1.2 or newer. Windows PowerShell 5.1 may offer only older ones: then TLS 1.2 is added, for this
    # session. One that leaves the choice to Windows (SystemDefault) is left alone, and a locked-down PowerShell that
    # refuses the change may still get through.
    try {
      $tls = [Net.ServicePointManager]::SecurityProtocol
      if ([int]$tls -ne 0 -and ([int]$tls -band [int][Net.SecurityProtocolType]::Tls12) -eq 0) {
        [Net.ServicePointManager]::SecurityProtocol = $tls -bor [Net.SecurityProtocolType]::Tls12
      }
    } catch { }

    if (-not $ref) {
      $url = Get-Setting 'RELEASES_URL'
      $builds = Get-Setting 'BUILDS_URL'
      if (-not $url -or -not $builds) {
        if (-not $github) {
          Say "$repo isn't on GitHub, so there is no latest release to ask it for: set HERSIR_RELEASES_URL and HERSIR_BUILDS_URL, or HERSIR_REF."
          Finish 1; return
        }
        if (-not $url) { $url = "https://api.github.com/repos/$github/releases/latest" }
      }
      Step "Finding Hersir's latest release"
      try {
        if ($url -like 'file:*') {
          $release = Get-Content -LiteralPath ([Uri]$url).LocalPath -Raw | ConvertFrom-Json
        } else {
          $release = Invoke-RestMethod -Uri $url -UseBasicParsing -TimeoutSec 60
        }
        $tag = "$($release.tag_name)"
      } catch {
        Say "Could not read the latest release from ${url}: $($_.Exception.Message)"
        Say 'Check your internet connection, then run this again.'
        Finish 1; return
      }
      if ($tag -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+$') {
        $shown = if ($tag) { $tag } else { 'missing' }
        Say "The latest release's tag, $shown, isn't a version like v1.2.3, so there is nothing to install."
        Finish 1; return
      }
      Say "Latest release: $tag"
      $version = $tag.Substring(1)
      $sig = "hersir-$version.sig"
      if (-not $builds) { $builds = "https://github.com/$github/releases/download/$tag" }
      $builds = $builds.TrimEnd('/')
      $tmp = Join-Path ([IO.Path]::GetTempPath()) ('hersir-get-' + [Guid]::NewGuid().ToString('N'))
      New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    }
    $what = if ($tag) { $tag } else { $ref }

    $dir = Join-Path $env:LOCALAPPDATA 'hersir-src'
    # The source folder from before 0.17.0, gizai-src, becomes hersir-src with its build, so the next build is quicker;
    # with a hersir-src there already (or when it can't be moved), it goes.
    $oldDir = Join-Path $env:LOCALAPPDATA 'gizai-src'
    if (Test-Path -LiteralPath $oldDir -PathType Container) {
      $moved = $false
      if (-not (Test-Path -LiteralPath $dir)) {
        try { Move-Item -LiteralPath $oldDir -Destination $dir; $moved = $true } catch { }
      }
      if (-not $moved) { Remove-Item -LiteralPath $oldDir -Recurse -Force -ErrorAction SilentlyContinue }
    }
    $gitDir = Join-Path $dir '.git'
    Step "Getting $what into $dir"
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    # git in the source folder's own repository only, never in one around it. A .git that git can't use (one cut
    # short) is made again.
    $git = @('-C', $dir, "--git-dir=$gitDir", "--work-tree=$dir")
    $null = Invoke-Shown 'git' ($git + @('rev-parse', '--git-dir'))
    if ($LASTEXITCODE -ne 0) {
      if (Test-Path -LiteralPath $gitDir) { Remove-Item -LiteralPath $gitDir -Recurse -Force }
      Invoke-Shown 'git' ($git + @('-c', 'init.defaultBranch=main', 'init', '--quiet'))
      if ($LASTEXITCODE -ne 0) { Say "Could not make a git repository in $dir."; Finish 1; return }
    }
    # As Hersir's own update: a shallow fetch of the tag alone, checked out as it is, and everything else in the folder
    # goes, except the build's target\, so the next build is quicker. A release is checked out only once its signature
    # checks, as the commit its tag points to: until then the folder stays as it was. No -- before the repository, as
    # get.sh has: Windows PowerShell may drop it.
    $spec = if ($ref) { $ref } else { "+refs/tags/${tag}:refs/tags/${tag}" }
    Invoke-Shown 'git' ($git + @('fetch', '--depth', '1', '--no-tags', '--force', $repo, $spec))
    if ($LASTEXITCODE -ne 0) { Say "Could not get $what from $repo."; Finish 1; return }
    if ($ref) {
      Say "$ref is a branch or a commit (HERSIR_REF), not a release: it has no signature to check, and is built from source."
      $checkout = 'FETCH_HEAD'
    } else {
      $commit = "$(Read-Output 'git' ($git + @('rev-parse', '--verify', '--quiet', "refs/tags/${tag}^{commit}")))".Trim()
      if ($LASTEXITCODE -ne 0 -or $commit -notmatch '^[0-9a-f]{40}$') { Say "Could not find the commit of $tag in $dir."; Finish 1; return }

      Step "Checking the signature of $tag"
      foreach ($file in @($sig, 'SHA256SUMS')) {
        $got = Get-File $file 60
        if ($got -eq 'missing') { Say "$tag isn't signed yet: try again in a few minutes."; Finish 1; return }
        if ($got -ne 'ok') { Say "Could not download $file of $tag from $builds ($got). Check your internet connection, then run this again."; Finish 1; return }
      }
      $signers = [Environment]::GetEnvironmentVariable('HERSIR_RELEASE_SIGNERS')
      if ($signers) {
        if (-not (Test-Path -LiteralPath $signers -PathType Leaf)) {
          Say "HERSIR_RELEASE_SIGNERS is $signers, which isn't a file, so the signature of $tag can't be checked."
          Finish 1; return
        }
        $signers = (Resolve-Path -LiteralPath $signers).ProviderPath
        Say "Checked against $signers (HERSIR_RELEASE_SIGNERS), not Hersir's release keys."
      } else {
        $signers = Join-Path $tmp 'release-signers'
        [IO.File]::WriteAllText($signers, ($ReleaseSigners.Replace("`r`n", "`n").Trim() + "`n"), (New-Object Text.UTF8Encoding $false))
      }
      $why = Test-Signature $signers
      if ($why) { Say $why; Finish 1; return }
      Say "$tag is signed with Hersir's release key, for its commit $commit."
      $checkout = $commit
    }
    Invoke-Shown 'git' ($git + @('checkout', '--quiet', '--force', '--detach', $checkout))
    if ($LASTEXITCODE -eq 0) { Invoke-Shown 'git' ($git + @('clean', '-ffdxq', '--exclude=/target/')) }
    if ($LASTEXITCODE -ne 0) { Say "Could not put $what in place in $dir."; Finish 1; return }
    $installer = Join-Path $dir 'install.ps1'
    if (-not (Test-Path -LiteralPath $installer)) {
      Say "$what has no install.ps1, so it can't be installed this way."
      Finish 1; return
    }

    $flags = @()
    if ($BuildOnly) { $flags += '-BuildOnly' }
    if ($SkipBuild) { $flags += '-SkipBuild' }
    if ($Uninstall) { $flags += '-Uninstall' }
    if ($Purge) { $flags += '-Purge' }
    # The release's ready-made build, installed with install.ps1 -SkipBuild. Not for -Uninstall, nor for your own
    # -SkipBuild (what is built in the source folder already) or -BuildOnly (a build).
    if (-not $ref -and -not $Uninstall -and -not $SkipBuild -and -not $BuildOnly) {
      if ($FromSource) {
        Say "$tag is built from source, as -FromSource asks."
      } else {
        $found = Find-ReadyMade
        if ($found.Why) {
          Say $found.Why
        } elseif ($Check) {
          Say "$tag has a ready-made build for Windows on x86_64, $($found.Archive): the check names what Hersir needs to run it."
          $flags += '-SkipBuild'
        } else {
          Step "Getting the ready-made build of $tag ($($found.Archive))"
          $why = Get-ReadyMade $found.Archive $found.Sum
          if ($why) {
            Say $why
          } else {
            Say "$($found.Archive) matches its checksum in SHA256SUMS, and is unpacked into $(Join-Path $dir 'target\release')."
            $flags += '-SkipBuild'
          }
        }
      }
    }
    # With Windows PowerShell, as Hersir's own update runs it, and Bypass, as the release's script isn't signed with a
    # certificate (its release is signed with Hersir's own key, checked above).
    $run = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $installer)
    # The release's check first, which says what is missing. Not before -Uninstall: install.ps1 -Check -Uninstall
    # would uninstall. What it finds missing (1) stops the install, also for a ready-made build, except with your own
    # -SkipBuild: install.ps1 installs what is built then too. Any other failure stops it.
    if (-not $Uninstall) {
      Invoke-Shown 'powershell.exe' ($run + @('-Check') + $flags)
      $code = $LASTEXITCODE
      if ($Check -or ($code -ne 0 -and $code -ne 1)) { Finish $code; return }
      if ($code -eq 1 -and -not $SkipBuild) {
        Say ''
        Say 'Install what is missing above, then run this again.'
        Finish 1; return
      }
    }
    Invoke-Shown 'powershell.exe' ($run + $flags)
    Finish $LASTEXITCODE
  } catch {
    Say "Hersir could not be installed: $($_.Exception.Message)"
    Finish 1
  } finally {
    if ($tmp) { Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue }
  }
} ($null -ne {}.File)
