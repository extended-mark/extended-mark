# Installs xtmd, Extended Mark's command, on Windows from the latest release: the x64 binary,
# checked against the release's SHA256SUMS, into %LOCALAPPDATA%\xtmd\bin, which it adds to your
# PATH. Windows 10 (1803) or later, for its tar.
#
#   irm https://raw.githubusercontent.com/extended-mark/extended-mark/main/install.ps1 | iex
#
#   $env:XTMD_VERSION = "0.3.0"      that release, not the latest
#   $env:XTMD_INSTALL_DIR = "C:\…"   where xtmd.exe goes
#   $env:XTMD_BASE_URL = "https://…" where the releases are (default the repository's on GitHub)
$ErrorActionPreference = "Stop"
# Invoke-WebRequest's progress bar slows a download many times over in Windows PowerShell
$ProgressPreference = "SilentlyContinue"
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$base = if ($env:XTMD_BASE_URL) { $env:XTMD_BASE_URL.TrimEnd("/") } else { "https://github.com/extended-mark/extended-mark/releases" }
$dir = if ($env:XTMD_INSTALL_DIR) { $env:XTMD_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "xtmd\bin" }

function Fail($why) { throw "xtmd install: $why" }

# x64 Windows, and Windows on Arm, which runs x64 programs
if (-not [Environment]::Is64BitOperatingSystem) { Fail "there is no xtmd for 32-bit Windows" }
$platform = "win32-x64"

$version = $env:XTMD_VERSION
if (-not $version) {
  # the latest release's page is its tag's: …/releases/tag/v0.3.0
  $req = [System.Net.HttpWebRequest]::Create("$base/latest")
  $req.AllowAutoRedirect = $false
  $req.Method = "HEAD"
  $res = $req.GetResponse()
  $location = $res.Headers["Location"]
  $res.Close()
  if ($location -notmatch "/tag/v?([^/]+)$") { Fail "couldn't find the latest release at $base/latest (set XTMD_VERSION to choose one)" }
  $version = $Matches[1]
}
$version = $version.TrimStart("v")

$asset = "xtmd-$version-$platform.tar.gz"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("xtmd-install-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  Write-Host "Installing xtmd $version for $platform..."
  Invoke-WebRequest -UseBasicParsing -Uri "$base/download/v$version/$asset" -OutFile (Join-Path $tmp $asset)
  Invoke-WebRequest -UseBasicParsing -Uri "$base/download/v$version/SHA256SUMS" -OutFile (Join-Path $tmp "SHA256SUMS")

  $want = $null
  foreach ($line in Get-Content (Join-Path $tmp "SHA256SUMS")) {
    $f = $line -split "\s+", 2
    if ($f.Count -eq 2 -and $f[1].TrimStart("*") -eq $asset) { $want = $f[0].ToLower() }
  }
  if (-not $want) { Fail "SHA256SUMS has no line for $asset" }
  $got = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $tmp $asset)).Hash.ToLower()
  if ($got -ne $want) { Fail "$asset doesn't match the release's SHA256SUMS: nothing installed" }

  tar -xzf (Join-Path $tmp $asset) -C $tmp
  if ($LASTEXITCODE -ne 0) { Fail "couldn't unpack $asset" }
  $exe = Join-Path $tmp "xtmd.exe"
  if (-not (Test-Path $exe)) { Fail "$asset has no xtmd.exe in it" }
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  Copy-Item -Force $exe (Join-Path $dir "xtmd.exe")
  # 0.3's frozen 0.2 -> 0.3 step, which `xtmd migrate` looks for beside the binary
  $step = Join-Path $tmp "migrate-0.2.cjs"
  if (Test-Path $step) { Copy-Item -Force $step (Join-Path $dir "migrate-0.2.cjs") }
} finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host "Installed xtmd $version at $(Join-Path $dir 'xtmd.exe')"
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not (($userPath -split ";") -contains $dir)) {
  $newPath = (@($userPath, $dir) | Where-Object { $_ }) -join ";"
  [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
  Write-Host "Added $dir to your PATH: open a new terminal to use xtmd."
}
# and in this session too
if (-not (($env:Path -split ";") -contains $dir)) { $env:Path = "$env:Path;$dir" }
