<#
.SYNOPSIS
  Installs prerequisites via Chocolatey (cmake, ninja, 7zip), initializes MSVC env,
  then configures, builds, installs llvm-project and package it.

.NOTES
  - First run should be in an elevated PowerShell (Admin) to allow choco installs.
  - If you already have an "x64 Native Tools for VS 2022" shell, the script will detect
    MSVC and skip manual env init. Otherwise it will try to init MSVC via vswhere.
#>

[CmdletBinding()]
param(
  [string]$RepoUrl = "https://github.com/llvm/llvm-project.git",
  [string]$Branch  = "llvmorg-21.1.0-rc3",
  [string]$Targets = "host",
  [string]$Projects = "clang;mlir",
  [string]$SourceDir = "$PWD\llvm-project",
  [string]$BuildDir  = "$PWD\llvm-build",
  [string]$InstallDir = "$PWD\llvm",
  [ValidateSet("Release","Debug","RelWithDebInfo","MinSizeRel")]
  [string]$Config = "Release",

  [bool]$AutoInstallVS = $true,
  [bool]$InstallVsWhere = $true,
  [bool]$InstallGit = $true,
  [bool]$InstallPython = $false
)

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
  # Requires Admin PowerShell
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

  # Refresh PATH for current session (new processes will already have it)
  $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" +
              [System.Environment]::GetEnvironmentVariable("Path","User")

  foreach ($tool in @("cmake","ninja")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
      throw "Missing tool after install: $tool (check PATH / restart shell)"
    }
  }
  Write-Host "CMake and Ninja are available." -ForegroundColor Green
}

function Detect-InNativeTools {
  # Heuristic: In Native Tools prompts, key MSVC vars are already set
  return ($env:VSCMD_ARG_TGT_ARCH -eq "x64" -or ($env:VisualStudioVersion -and $env:VSCMD_VER))
}

function Import-EnvFromCmd($cmdLine) {
    # Create a temp .bat that CALLs the given cmdLine, then prints env via 'set'
    $tmp = New-TemporaryFile
    try {
        $bat = "$($tmp.FullName).bat"
        # Important: use 'call' so control returns to this .bat and 'set' runs
        $content = @"
@echo off
call $cmdLine
set
"@
        Set-Content -Path $bat -Value $content -Encoding ASCII

        # Capture environment from the batch
        $envDump = & cmd.exe /c `"$bat`"

        # Import into current PS process
        $envDump -split "`r?`n" | ForEach-Object {
            if ($_ -match "^(.*?)=(.*)$") {
                $name = $Matches[1]
                $value = $Matches[2]
                if ($name -ieq "PATH") {
                    $env:Path = $value
                } else {
                    [System.Environment]::SetEnvironmentVariable($name, $value, "Process")
                }
            }
        }
    } finally {
        Remove-Item -Force -ErrorAction SilentlyContinue $tmp, $bat
    }
}

function Ensure-MsvcEnv {
  if (Detect-InNativeTools) {
    Write-Host "MSVC environment already initialized (Native Tools shell detected)." -ForegroundColor Green
    return
  }

  Write-Section "Initializing MSVC build environment via vswhere"

  # Resolve vswhere (PS 5.1 safe)
  $vswhereCmd = Get-Command vswhere -ErrorAction SilentlyContinue
  $vswhere = if ($vswhereCmd) { $vswhereCmd.Path } else { $null }

  if (-not $vswhere) {
    if ($AutoInstallVS) {
      Write-Host "vswhere not found, installing via Chocolatey..." -ForegroundColor Yellow
      Choco-Install @("vswhere")
      $vswhereCmd = Get-Command vswhere -ErrorAction SilentlyContinue
      $vswhere = if ($vswhereCmd) { $vswhereCmd.Path } else { $null }
    }
    if (-not $vswhere) {
      throw "vswhere not found. Re-run with -AutoInstallVS:`$true or install Visual Studio 2022 (Build Tools or Community) and retry."
    }
  }

  # Try to find a VS install with C++ tools
  $vsInstall = & $vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath

  if (-not $vsInstall) {
    if ($AutoInstallVS) {
      Write-Section "Installing Visual Studio 2022 Build Tools (C++ workload)"
      # Big install; requires Admin shell
      # core build tools + vctools workload (MSVC, Windows SDK, etc.)
      Choco-Install @(
        "visualstudio2022buildtools",
        "visualstudio2022-workload-vctools"
      )
      # Try query again
      $vsInstall = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    }
  }

  if (-not $vsInstall) {
    throw "No Visual Studio with C++ tools found. Install VS 2022 (Desktop C++ or Build Tools) and retry."
  }

  Write-Host "Found Visual Studio at: $vsInstall" -ForegroundColor Green

  # Prefer VsDevCmd.bat; fall back to vcvarsall.bat
  $vsDevCmd = Join-Path $vsInstall "Common7\Tools\VsDevCmd.bat"
  $vcVarsAll = Join-Path $vsInstall "VC\Auxiliary\Build\vcvarsall.bat"

  if (Test-Path $vsDevCmd) {
    Import-EnvFromCmd "`"$vsDevCmd`" -arch=x64"
  } elseif (Test-Path $vcVarsAll) {
    Import-EnvFromCmd "`"$vcVarsAll`" x64"
  } else {
    throw "Could not find VsDevCmd.bat or vcvarsall.bat under $vsInstall"
  }

  # Verify MSVC actually ready
  $cl = Get-Command cl.exe -ErrorAction SilentlyContinue
  if (-not $cl) {
    throw "MSVC 'cl.exe' not on PATH after init, environment init failed."
  }

  # Optional: show version for debug
  cmd /c "cl.exe /Bv" | Out-Host
  Write-Host "MSVC environment initialized." -ForegroundColor Green
}

function Ensure-Git {
  if (Get-Command git -ErrorAction SilentlyContinue) { return }
  if ($InstallGit) {
    Choco-Install @("git")
    return
  }
  throw "Git is required but not found. Re-run with -InstallGit:$true or install Git manually."
}

# ---------------- MAIN ----------------
Write-Section "Chocolatey setup"
Ensure-Choco

Write-Section "Prerequisites"
Ensure-Tools
Ensure-Git
Ensure-MsvcEnv

Write-Section "Clone or update llvm-project"
if (-not (Test-Path $SourceDir)) {
  Exec "git clone --depth 1 --branch $Branch `"$RepoUrl`" `"$SourceDir`"" "git clone failed"
} else {
  Push-Location $SourceDir
  try {
    Exec "git fetch --tags origin $Branch" "git fetch failed"
    Exec "git checkout $Branch" "git checkout failed"
    Exec "git pull --ff-only" "git pull failed"
  } finally { Pop-Location }
}

$jobs = $env:NUMBER_OF_PROCESSORS

Write-Section "Configure (CMake + Ninja)"
New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null
$cmakeConfigure = @(
    "cmake",
    "-S `"$SourceDir\llvm`"",
    "-B `"$BuildDir`"",
    "-G Ninja",
    "-DCMAKE_BUILD_TYPE=$Config",
    "-DBUILD_SHARED_LIBS=OFF",
    "-DLLVM_ENABLE_PROJECTS=`"$Projects`"",
    "-DLLVM_TARGETS_TO_BUILD=`"$Targets`"",
    "-DLLVM_BUILD_TESTS=OFF",
    "-DLLVM_INCLUDE_TESTS=OFF",
    "-DLLVM_BUILD_EXAMPLES=OFF",
    "-DLLVM_INCLUDE_EXAMPLES=OFF",
    "-DLLVM_BUILD_DOCS=OFF",
    "-DLLVM_ENABLE_DOXYGEN=OFF",
    "-DLLVM_ENABLE_LTO=OFF",
    "-DLLVM_ENABLE_SPHINX=OFF",
    "-DLLVM_STATIC_LINK_CXX_STDLIB=ON",
    "-DLLVM_ENABLE_ZLIB=OFF",
    "-DLLVM_ENABLE_LIBXML2=OFF",
    "-DLLVM_ENABLE_LIBEDIT=OFF",
    "-DLLVM_ENABLE_PER_TARGET_RUNTIME_DIR=ON",
    "-DLLVM_PARALLEL_LINK_JOBS=$jobs",
    "-DLLVM_INSTALL_TOOLCHAIN_ONLY=ON",
    "-DCMAKE_INSTALL_PREFIX=`"$InstallDir`"",
    "-DCMAKE_CXX_FLAGS=/bigobj -DCMAKE_C_FLAGS=/bigobj"
) -join " "
Exec $cmakeConfigure "CMake configure failed"
$buildNinja = Join-Path $BuildDir "build.ninja"
if (-not (Test-Path $buildNinja)) { throw "Configure failed (no build.ninja generated)" }

# Build (and install)
Exec "cmake --build `"$BuildDir`" --config $Config -- -j $jobs -v -k 0" "Build failed"
Exec "cmake --install `"$BuildDir`" --config $Config" "Install failed"
$llvmConfig = Join-Path $InstallDir "bin\llvm-config.exe"
if (-not (Test-Path $llvmConfig)) { throw "Install failed (llvm-config.exe not found)" }

Write-Host "Done. LLVM/Clang installed to: $InstallDir" -ForegroundColor Green
Write-Host "Add to PATH: $InstallDir\bin" -ForegroundColor Yellow

# Post-install: keep only llvm-config in bin
$osName = (Get-CimInstance Win32_OperatingSystem).Caption
$configName = "llvm-config.exe"  # we’re on Windows
$binDir = Join-Path $InstallDir "bin"
$cfgPath = Join-Path $binDir $configName

# only package if install produced llvm-config
if (Test-Path $llvmConfig) {
    $parent = Split-Path $InstallDir -Parent
    $leaf   = Split-Path $InstallDir -Leaf
    $tar    = "windows-x64.tar"
    $xz     = "$tar.xz"

    if (Test-Path $tar) { Remove-Item -Force $tar }
    if (Test-Path $xz)  { Remove-Item -Force $xz }

    Exec "tar -C `"$parent`" -cf `"$tar`" `"$leaf`"" "tar create failed"
    Exec "7z a -txz `"$xz`" `"$tar`"" "xz compress failed"
    Remove-Item -Force $tar

    Write-Host "Created package: $xz" -ForegroundColor Green
}

# Package as tar.xz
$platform = "windows-x64"
$cwd = Get-Location

Push-Location (Split-Path $InstallDir -Parent)
try {
  # Create tar from 'llvm' directory, then compress to .xz using 7z
  if ((Split-Path $InstallDir -Leaf) -ne "llvm") {
    Write-Host "Note: InstallDir is '$InstallDir'. Bash script uses '../llvm' as name. Packaging current InstallDir."
  }

  $tarName = "$platform.tar"
  $xzName  = "$platform.tar.xz"

  if (Test-Path $tarName) { Remove-Item -Force $tarName }
  if (Test-Path $xzName)  { Remove-Item -Force $xzName }

  # Create tar of the install directory name (folder itself)
  $installLeaf = Split-Path $InstallDir -Leaf
  Exec "tar -cf `"$tarName`" `"$installLeaf`"" "tar create failed"

  # Compress to .xz using 7z
  Exec "7z a -txz `"$xzName`" `"$tarName`"" "7z xz failed"
  Remove-Item -Force $tarName

  Write-Host "Created package: $xzName" -ForegroundColor Green
} finally {
  Pop-Location
}
