<#
.SYNOPSIS
  Installs prerequisites via Chocolatey (cmake, ninja, 7zip), initializes MSVC env,
  then configures, builds, installs llvm-project and packages it, using a .llvm workspace.

.USAGE
  pwsh -File build-llvm.ps1 20.1.4 1
#>

[CmdletBinding()]
param(
  # positional
  [Parameter(Mandatory=$true, Position=0)]
  [string]$Version,
  [Parameter(Mandatory=$true, Position=1)]
  [string]$ReleaseNumber,

  # optional
  [string]$RepoUrl   = "https://github.com/llvm/llvm-project.git",
  [string]$Targets   = "host",
  [string]$Projects  = "clang;mlir",

  [string]$Workspace = (Join-Path $PSScriptRoot ".llvm"),

  [ValidateSet("Release","Debug","RelWithDebInfo","MinSizeRel")]
  [string]$Config = "Release",

  [bool]$AutoInstallVS = $true,
  [bool]$InstallVsWhere = $true,
  [bool]$InstallGit = $true,
  [bool]$InstallPython = $true
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$Branch = "llvmorg-$Version"
$PkgDir = "tracel-llvm-$Version-$ReleaseNumber"

# workspace layout
$SourceDir  = Join-Path $Workspace "llvm-project"
$BuildDir   = Join-Path $Workspace "build"
$InstallDir = Join-Path $Workspace $PkgDir

function Write-Section($text) {
  Write-Host "`n=== $text ===" -ForegroundColor Magenta
}

function Exec($cmd, $errMsg) {
  Write-Host ">> $cmd" -ForegroundColor Cyan
  $global:LASTEXITCODE = 0
  & cmd.exe /c $cmd
  if ($LASTEXITCODE -ne 0) {
    throw "$errMsg (exit $LASTEXITCODE)"
  }
}

function Ensure-Choco {
  if (Get-Command choco -ErrorAction SilentlyContinue) {
    Write-Host "Chocolatey found." -ForegroundColor Green
    return
  }
  Write-Section "Installing Chocolatey"
  Set-ExecutionPolicy Bypass -Scope Process -Force | Out-Null
  [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12
  $script = (New-Object Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1')
  Invoke-Expression $script
  if (-not (Get-Command choco -ErrorAction SilentlyContinue)) {
    throw "Chocolatey installation failed or not on PATH yet. Restart an elevated PowerShell and retry."
  }
  Write-Host "Chocolatey installed." -ForegroundColor Green
}

function Choco-Install($packages) {
  $pkgStr = ($packages -join ' ')
  Exec "choco install $pkgStr --yes --no-progress" "Chocolatey install failed for: $pkgStr"
}

function Ensure-Tools {
  Write-Section "Installing prerequisites via Chocolatey"
  $toInstall = @("cmake","ninja","7zip")
  if ($InstallVsWhere) { $toInstall += "vswhere" }
  if ($InstallGit)     { $toInstall += "git" }
  if ($InstallPython)  { $toInstall += "python" }
  Choco-Install $toInstall

  $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" +
              [System.Environment]::GetEnvironmentVariable("Path","User")

  foreach ($tool in @("cmake","ninja","7z")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
      throw "Missing tool after install: $tool (check PATH / restart shell)"
    }
  }
  Write-Host "CMake, Ninja, and 7-Zip are available." -ForegroundColor Green
}

function Detect-InNativeTools {
  return ($env:VSCMD_ARG_TGT_ARCH -eq "x64" -or ($env:VisualStudioVersion -and $env:VSCMD_VER))
}

function Import-EnvFromCmd($cmdLine) {
  $tmp = New-TemporaryFile
  try {
    $bat = "$($tmp.FullName).bat"
    $content = @"
@echo off
call $cmdLine
set
"@
    Set-Content -Path $bat -Value $content -Encoding ASCII
    $envDump = & cmd.exe /c `"$bat`"
    $envDump -split "`r?`n" | ForEach-Object {
      if ($_ -match "^(.*?)=(.*)$") {
        $name = $Matches[1]; $value = $Matches[2]
        if ($name -ieq "PATH") { $env:Path = $value }
        else { [System.Environment]::SetEnvironmentVariable($name, $value, "Process") }
      }
    }
  } finally {
    Remove-Item -Force -ErrorAction SilentlyContinue $tmp, $bat
  }
}

function Show-CompilerBanner {
    Write-Host "Showing MSVC toolchain banner..." -ForegroundColor DarkCyan
    cmd /v:on /c "set CL=& cl.exe /Bv & exit /b 0" | Out-Host
}

function Ensure-MsvcEnv {
  if (Detect-InNativeTools) {
    Write-Host "MSVC environment already initialized (Native Tools shell detected)." -ForegroundColor Green
    return
  }

  Write-Section "Initializing MSVC build environment via vswhere"

  $vswhereCmd = Get-Command vswhere -ErrorAction SilentlyContinue
  $vswhere    = if ($vswhereCmd) { $vswhereCmd.Path } else { $null }

  if (-not $vswhere -and $InstallVsWhere) {
    Write-Host "vswhere not found, installing..." -ForegroundColor Yellow
    Choco-Install @("vswhere")
    $vswhereCmd = Get-Command vswhere -ErrorAction SilentlyContinue
    $vswhere    = if ($vswhereCmd) { $vswhereCmd.Path } else { $null }
  }

  if (-not $vswhere) {
    throw "vswhere not found. Install Visual Studio 2022 Build Tools or run with -AutoInstallVS:`$true."
  }

  # Query VS install
  $vsInstall = & $vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath

  # Install VS Build Tools if missing
  if (-not $vsInstall -and $AutoInstallVS) {
    Write-Section "Installing Visual Studio 2022 Build Tools (C++ workload)"
    Choco-Install @("visualstudio2022buildtools","visualstudio2022-workload-vctools")
    # Re-query
    $vsInstall = & $vswhere -latest -products * `
      -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
      -property installationPath
  }

  if (-not $vsInstall) {
    throw "No Visual Studio with C++ tools found. The script cannot continue."
  }

  Write-Host "Found Visual Studio at: $vsInstall" -ForegroundColor Green

  $vsDevCmd  = Join-Path $vsInstall "Common7\Tools\VsDevCmd.bat"
  $vcVarsAll = Join-Path $vsInstall "VC\Auxiliary\Build\vcvarsall.bat"

  if (Test-Path $vsDevCmd)      { Import-EnvFromCmd "`"$vsDevCmd`" -arch=x64" }
  elseif (Test-Path $vcVarsAll) { Import-EnvFromCmd "`"$vcVarsAll`" x64" }
  else                          { throw "Could not find VsDevCmd.bat or vcvarsall.bat under $vsInstall" }

  # sanity check
  if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    throw "MSVC 'cl.exe' not on PATH after init."
  }
  Show-CompilerBanner
  Write-Host "MSVC environment initialized." -ForegroundColor Green
}

function Ensure-Git {
  if (Get-Command git -ErrorAction SilentlyContinue) { return }
  if ($InstallGit) { Choco-Install @("git"); return }
  throw "Git is required but not found. Re-run with -InstallGit:$true or install Git manually."
}

# ---------------- MAIN ----------------
Write-Section "Chocolatey setup"
Ensure-Choco

Write-Section "Prerequisites"
Ensure-Tools
Ensure-Git
Ensure-MsvcEnv

Write-Section "Prepare .llvm workspace"
New-Item -ItemType Directory -Path $Workspace -Force | Out-Null

# Clean old workspace
$oldDirs = @(
  (Join-Path $Workspace $PkgDir),
  (Join-Path $Workspace "llvm-project"),
  (Join-Path $Workspace "build")
)
foreach ($d in $oldDirs) {
  if (Test-Path $d) { Remove-Item -Recurse -Force $d }
}

Write-Section "Clone llvm-project (fresh in workspace)"
Exec "git clone --depth 1 --branch $Branch `"$RepoUrl`" `"$SourceDir`"" "git clone failed"

New-Item -ItemType Directory -Path 'C:\Temp' -Force | Out-Null
$env:TEMP = 'C:\Temp'
$env:TMP  = 'C:\Temp'
$jobs = $env:NUMBER_OF_PROCESSORS

# Pick the CRT matching your $Config (Release/Debug):
$msvcCRT = if ($Config -eq "Debug") { "MultiThreadedDebugDLL" } else { "MultiThreadedDLL" }

Write-Section "Configure (CMake + Ninja)"
New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null
$cmakeConfigure = @(
  "cmake",
  "-S `"$SourceDir\llvm`"",
  "-B `"$BuildDir`"",
  "-G Ninja",
  "-DLLVM_PARALLEL_LINK_JOBS=$jobs",
  "-DCMAKE_BUILD_TYPE=$Config",
  "-DCMAKE_INSTALL_PREFIX=`"$InstallDir`"",
  "-DBUILD_SHARED_LIBS=OFF",
  "-DLLVM_ENABLE_PROJECTS=`"$Projects`"",
  "-DLLVM_TARGETS_TO_BUILD=`"$Targets`"",
  "-DLLVM_INCLUDE_TOOLS=ON",
  "-DLLVM_BUILD_TOOLS=OFF",
  "-DLLVM_BUILD_TESTS=OFF",
  "-DLLVM_BUILD_EXAMPLES=OFF",
  "-DLLVM_INCLUDE_TESTS=OFF",
  "-DLLVM_INCLUDE_DOCS=OFF",
  "-DLLVM_INCLUDE_EXAMPLES=OFF",
  "-DLLVM_ENABLE_DIA_SDK=OFF",
  "-DLLVM_ENABLE_DOXYGEN=OFF",
  "-DLLVM_ENABLE_SPHINX=OFF",
  "-DLLVM_ENABLE_ZLIB=OFF",
  "-DLLVM_ENABLE_LIBXML2=OFF",
  "-DLLVM_ENABLE_LIBEDIT=OFF",
  "-DLLVM_ENABLE_LTO=OFF",
  "-DLLVM_ENABLE_RTTI=ON",
  "-DLLVM_ENABLE_DUMP=ON",
  "-DLLVM_ENABLE_PER_TARGET_RUNTIME_DIR=ON",
  "-DCMAKE_MSVC_DEBUG_INFORMATION_FORMAT=`"`"",
  "-DCMAKE_MSVC_RUNTIME_LIBRARY=$msvcCRT"
) -join " "
Exec $cmakeConfigure "CMake configure failed"
$buildNinja = Join-Path $BuildDir "build.ninja"
if (-not (Test-Path $buildNinja)) { throw "Configure failed (no build.ninja generated)" }

Write-Section "Build and Install"
Exec "cmake --build `"$BuildDir`" --config $Config -- -j $jobs -v -k 0" "Build failed"
Exec "cmake --build `"$BuildDir`" --target llvm-config --config $Config -- -j $jobs -v -k 0" "Build llvm-config failed"
Exec "cmake --install `"$BuildDir`" --config $Config" "Install failed"

Write-Host "Installed to: $InstallDir" -ForegroundColor Green

Write-Section "Post-install cleanup"
$installBin = Join-Path $InstallDir 'bin'
$installInclude = Join-Path $InstallDir 'include'
$installLib = Join-Path $InstallDir 'lib'
# Keep only llvm-config.exe and libclang.dll
Remove-Item -Recurse -Force $installBin
New-Item -ItemType Directory -Path $installBin -Force | Out-Null
Copy-Item (Join-Path $BuildDir "bin\llvm-config.exe") $installBin -Force
Copy-Item (Join-Path $BuildDir "bin\libclang.dll") $installBin -Force
Write-Host "include..."
$installInclude = Join-Path $InstallDir 'include'
Remove-Item -Recurse -Force (Join-Path $installInclude "clang") -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force (Join-Path $installInclude "clang-c") -ErrorAction SilentlyContinue
Write-Host "lib..."
$installLib = Join-Path $InstallDir 'lib'
Remove-Item -Recurse -Force (Join-Path $installLib "clang") -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force (Join-Path $installLib "libear") -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force (Join-Path $installLib "libscanbuild") -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force (Join-Path $installLib "objects-Release") -ErrorAction SilentlyContinue
Remove-Item -Force (Join-Path $installLib "clang*.lib") -ErrorAction SilentlyContinue
Remove-Item -Force (Join-Path $installLib "mlir_*.lib") -ErrorAction SilentlyContinue
Remove-Item -Force (Join-Path $installLib "LTO.lib") -ErrorAction SilentlyContinue
Remove-Item -Force (Join-Path $installLib "Remarks.lib") -ErrorAction SilentlyContinue
Remove-Item -Force (Join-Path $installLib "mlir_*runner_utils*.lib") -ErrorAction SilentlyContinue
Remove-Item -Force (Join-Path $installLib "mlir_*c_runner_utils*.lib") -ErrorAction SilentlyContinue
Remove-Item -Force (Join-Path $installLib "mlir_*async*runtime*.lib") -ErrorAction SilentlyContinue
Write-Host "others..."
Remove-Item -Recurse -Force (Join-Path $InstallDir "libexec")
Remove-Item -Recurse -Force (Join-Path $InstallDir "share")
# Strip PDBs everywhere (bin + lib)
Get-ChildItem -Path $InstallDir -Recurse -Include *.pdb -File -ErrorAction SilentlyContinue | Remove-Item -Force

# Platform string for artifact name
$arch = switch -Regex ($env:PROCESSOR_ARCHITECTURE) {
  "ARM64" { "AArch64"; break }
  "AMD64" { "x64"; break }
  default { $env:PROCESSOR_ARCHITECTURE }
}
$platform = "windows-$arch"

# Write-Section "Package from workspace (.tar.xz)"
Push-Location $Workspace
$tarName = "$platform.tar"
$xzName  = "$platform.tar.xz"
try {
    if (Test-Path $tarName) { Remove-Item -Force $tarName }
    if (Test-Path $xzName)  { Remove-Item -Force $xzName }

    # Create tar with *folder named llvm-$Version-$ReleaseNumber* at top-level
    Exec "tar -cf `"$tarName`" `"$PkgDir`"" "tar create failed"
    Exec "7z a -txz `"$xzName`" `"$tarName`"" "7z xz failed"
    Remove-Item -Force $tarName

    Write-Host "Created package: $(Join-Path $Workspace $xzName)" -ForegroundColor Green
} finally {
    Pop-Location
}

# Checksum sidecar files
Write-Section "Compute checksums and write sidecar JSON"
# Archive SHA-256
$archivePath   = Join-Path $Workspace $xzName
if (-not (Test-Path $archivePath)) { throw "Archive not found: $archivePath" }

function Get-DirectoryContentSha256([string]$root) {
  $sha = [System.Security.Cryptography.SHA256]::Create()
  try {
    $root = [System.IO.Path]::GetFullPath($root).TrimEnd('\','/')

    # Collect (Rel, Full, Key) where Key = hex of UTF-8 bytes of Rel (for bytewise sort)
    $items = [System.Collections.Generic.List[object]]::new()
    Get-ChildItem -LiteralPath $root -Recurse -File -Force | ForEach-Object {
      $full = $_.FullName
      $rel = $null
      try {
        # Works on newer PowerShell/.NET
        $rel = [System.IO.Path]::GetRelativePath($root, $full)
      } catch {
        # Fallback for older Windows PowerShell
        $rel = $full.Substring($root.Length).TrimStart('\','/')
      }
      $rel = ($rel -replace '\\','/')
      $relBytes = [System.Text.Encoding]::UTF8.GetBytes($rel)
      $keyHex   = ([System.BitConverter]::ToString($relBytes)).Replace('-', '').ToLowerInvariant()
      $items.Add([PSCustomObject]@{ Rel = $rel; Full = $full; Key = $keyHex })
    }

    # Sort by UTF-8 bytes (via the hex key)
    $items = $items | Sort-Object -Property Key -CaseSensitive

    foreach ($it in $items) {
      # PATH\n  (UTF-8)
      $pathBytes = [System.Text.Encoding]::UTF8.GetBytes($it.Rel + "`n")
      $sha.TransformBlock($pathBytes, 0, $pathBytes.Length, $null, 0) | Out-Null

      # SIZE\n  (ASCII digits, invariant)
      $lenStr = ([System.IO.FileInfo]$it.Full).Length.ToString([System.Globalization.CultureInfo]::InvariantCulture) + "`n"
      $lenBytes = [System.Text.Encoding]::UTF8.GetBytes($lenStr)
      $sha.TransformBlock($lenBytes, 0, $lenBytes.Length, $null, 0) | Out-Null

      # BYTES
      $fs = [System.IO.File]::Open($it.Full, 'Open', 'Read', 'Read')
      try {
        $buffer = New-Object byte[] (1024*1024)
        while (($read = $fs.Read($buffer, 0, $buffer.Length)) -gt 0) {
          $sha.TransformBlock($buffer, 0, $read, $null, 0) | Out-Null
        }
      } finally { $fs.Dispose() }
    }

    $sha.TransformFinalBlock([byte[]]::new(0), 0, 0) | Out-Null
    ($sha.Hash | ForEach-Object { $_.ToString('x2') }) -join ''
  } finally {
    $sha.Dispose()
  }
}
$archiveSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $archivePath).Hash.ToLower()
$contentSha256 = Get-DirectoryContentSha256 $InstallDir

# Sidecar manifest
$manifest = [PSCustomObject]@{
  version        = $Version
  release_number = $ReleaseNumber
  platform       = $platform
  created_at_utc = (Get-Date).ToUniversalTime().ToString("o")
  archive_sha256 = $archiveSha256
  content_sha256 = $contentSha256
}
$sidecarPath = Join-Path $Workspace "$platform.checksums.json"
$manifest | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $sidecarPath -Encoding utf8
Write-Host "Archive sha256: $archiveSha256" -ForegroundColor DarkCyan
Write-Host "Content sha256: $contentSha256" -ForegroundColor DarkCyan
Write-Host "Sidecar manifest: $sidecarPath" -ForegroundColor Green

Write-Host "=== LLVM build and packaging completed successfully! ===" -ForegroundColor Green
Write-Host "Workspace: $Workspace" -ForegroundColor Yellow
Write-Host "Install dir: $InstallDir" -ForegroundColor Yellow
Write-Host "Package: $(Join-Path $Workspace "$platform.tar.xz")" -ForegroundColor Yellow
