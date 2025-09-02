<#
.SYNOPSIS
  Installs prerequisites via Chocolatey (cmake, ninja, 7zip), initializes MSVC env,
  then configures, builds, installs llvm-project and packages it, using a .llvm workspace.

.NOTES
  - First run should be in an elevated PowerShell (Admin) to allow choco installs.
  - If you already have an "x64 Native Tools for VS 2022" shell, the script will detect
    MSVC and skip manual env init. Otherwise it will try to init MSVC via vswhere.
#>

[CmdletBinding()]
param(
  [string]$Version = "21.1.0-rc3",

  [string]$RepoUrl = "https://github.com/llvm/llvm-project.git",
  [string]$Targets = "host",
  [string]$Projects = "clang;mlir",
  [string]$Workspace = "$PWD\.llvm",
  [ValidateSet("Release","Debug","RelWithDebInfo","MinSizeRel")]
  [string]$Config = "Release",

  [bool]$AutoInstallVS = $true,
  [bool]$InstallVsWhere = $true,
  [bool]$InstallGit = $true,
  [bool]$InstallPython = $true
)
$Branch = "llvmorg-$Version"

# workspace layout
$SourceDir  = Join-Path $Workspace "llvm-project"
$BuildDir   = Join-Path $Workspace "build"
$InstallDir = Join-Path $Workspace "llvm"

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

  foreach ($tool in @("cmake","ninja")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
      throw "Missing tool after install: $tool (check PATH / restart shell)"
    }
  }
  Write-Host "CMake and Ninja are available." -ForegroundColor Green
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

function Ensure-MsvcEnv {
  if (Detect-InNativeTools) {
    Write-Host "MSVC environment already initialized (Native Tools shell detected)." -ForegroundColor Green
    return
  }

  Write-Section "Initializing MSVC build environment via vswhere"

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

  $vsInstall = & $vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath

  if (-not $vsInstall) {
    if ($AutoInstallVS) {
      Write-Section "Installing Visual Studio 2022 Build Tools (C++ workload)"
      Choco-Install @("visualstudio2022buildtools","visualstudio2022-workload-vctools")
      $vsInstall = & $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    }
  }

  if (-not $vsInstall) {
    throw "No Visual Studio with C++ tools found. Install VS 2022 (Desktop C++ or Build Tools) and retry."
  }

  Write-Host "Found Visual Studio at: $vsInstall" -ForegroundColor Green

  $vsDevCmd = Join-Path $vsInstall "Common7\Tools\VsDevCmd.bat"
  $vcVarsAll = Join-Path $vsInstall "VC\Auxiliary\Build\vcvarsall.bat"

  if (Test-Path $vsDevCmd)      { Import-EnvFromCmd "`"$vsDevCmd`" -arch=x64" }
  elseif (Test-Path $vcVarsAll) { Import-EnvFromCmd "`"$vcVarsAll`" x64" }
  else                          { throw "Could not find VsDevCmd.bat or vcvarsall.bat under $vsInstall" }

  $cl = Get-Command cl.exe -ErrorAction SilentlyContinue
  if (-not $cl) { throw "MSVC 'cl.exe' not on PATH after init, environment init failed." }

  cmd /c "cl.exe /Bv" | Out-Host
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
  (Join-Path $Workspace "llvm"),
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
    "-DLLVM_ENABLE_DIA_SDK=OFF",
    "-DLLVM_ENABLE_DOXYGEN=OFF",
    "-DLLVM_ENABLE_LTO=OFF",
    "-DLLVM_ENABLE_SPHINX=OFF",
    "-DLLVM_STATIC_LINK_CXX_STDLIB=ON",
    "-DLLVM_ENABLE_ZLIB=OFF",
    "-DLLVM_ENABLE_LIBXML2=OFF",
    "-DLLVM_ENABLE_LIBEDIT=OFF",
    "-DLLVM_ENABLE_PER_TARGET_RUNTIME_DIR=ON",
    "-DLLVM_PARALLEL_LINK_JOBS=$jobs",
    "-DCMAKE_INSTALL_PREFIX=`"$InstallDir`"",
    "-DCMAKE_CXX_FLAGS=/bigobj -DCMAKE_C_FLAGS=/bigobj"
) -join " "
Exec $cmakeConfigure "CMake configure failed"
$buildNinja = Join-Path $BuildDir "build.ninja"
if (-not (Test-Path $buildNinja)) { throw "Configure failed (no build.ninja generated)" }

Write-Section "Build and Install"
Exec "cmake --build `"$BuildDir`" --config $Config -- -j $jobs -v -k 0" "Build failed"
Exec "cmake --install `"$BuildDir`" --config $Config" "Install failed"
$llvmConfig = Join-Path $InstallDir "bin\llvm-config.exe"
if (-not (Test-Path $llvmConfig)) { throw "Install failed (llvm-config.exe not found)" }

Write-Host "Installed to: $InstallDir" -ForegroundColor Green

Write-Section "Post-install cleanup (keep only llvm-config in bin)"
$installBin = Join-Path $InstallDir 'bin'
$cfgName    = 'llvm-config.exe'
$cfgPath    = Join-Path $installBin $cfgName
if (-not (Test-Path $cfgPath)) { throw "Install finished but $cfgName not found in $installBin." }
$stash = Join-Path $InstallDir $cfgName
Move-Item $cfgPath $stash -Force
Get-ChildItem $installBin -Force | Remove-Item -Recurse -Force
Move-Item $stash $installBin -Force

Write-Section "Package from workspace (.tar.xz)"
$platform = "windows-x64"

Push-Location $Workspace
try {
  $tarName = "$platform.tar"
  $xzName  = "$platform.tar.xz"
  if (Test-Path $tarName) { Remove-Item -Force $tarName }
  if (Test-Path $xzName)  { Remove-Item -Force $xzName }
  # Create tar of the 'llvm' directory (folder itself)
  Exec "tar -cf `"$tarName`" `"$([IO.Path]::GetFileName($InstallDir))`"" "tar create failed"
  # Compress to .xz using 7z
  Exec "7z a -txz `"$xzName`" `"$tarName`"" "7z xz failed"
  Remove-Item -Force $tarName

  Write-Host "Created package: $(Join-Path $Workspace $xzName)" -ForegroundColor Green
} finally {
  Pop-Location
}

Write-Host "=== LLVM build and packaging completed successfully! ===" -ForegroundColor Green
Write-Host "Workspace: $Workspace" -ForegroundColor Yellow
Write-Host "Install dir: $InstallDir" -ForegroundColor Yellow
Write-Host "Package: $(Join-Path $Workspace "$platform.tar.xz")" -ForegroundColor Yellow
