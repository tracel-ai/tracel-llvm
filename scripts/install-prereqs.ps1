<#
.SYNOPSIS
  Install prerequisites to build LLVM and generate bindings.

.USAGE
  pwsh -File install-prereqs.ps1
#>

$ErrorActionPreference = "Stop"

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
    $envDump = & cmd.exe /c "`"$bat`""
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

function Ensure-VsWhere {
  if (Get-Command vswhere -ErrorAction SilentlyContinue) {
    return
  }
  $dst = Join-Path $env:TEMP "vswhere.exe"
  Write-Host "Downloading vswhere..." -ForegroundColor Cyan
  Invoke-WebRequest -Uri "https://github.com/microsoft/vswhere/releases/latest/download/vswhere.exe" -OutFile $dst
  if (-not (Test-Path $dst)) {
    throw "vswhere should be downloaded"
  }
  # Put it on PATH for this process and subsequent steps in CI
  $dir = Split-Path $dst -Parent
  $env:PATH = "$dir;$env:PATH"
  if ($env:GITHUB_ACTIONS -eq "true") {
    $dir | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
  }
}

function Ensure-VSBuildTools {
  Ensure-VsWhere

  $vsInstall = & vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath

  if ($vsInstall) {
    Write-Host "Found Visual Studio Build Tools at: $vsInstall" -ForegroundColor Green
    return
  }

  if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    throw "winget should be available to install Visual Studio Build Tools on this runner"
  }

  Write-Host "Installing Visual Studio 2022 Build Tools (C++ toolchain) via winget..." -ForegroundColor Cyan

  # This installs VS Build Tools. Components are added in the next step.
  & winget install --id Microsoft.VisualStudio.2022.BuildTools -e --source winget `
    --accept-package-agreements --accept-source-agreements

  # Add required components: MSVC toolset + Windows SDK (adjust versions if you need)
  $vsInstaller = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vs_installer.exe"
  if (-not (Test-Path $vsInstaller)) {
    throw "vs_installer.exe should exist after installing Build Tools"
  }

  # Modify Build Tools to include C++ build tools and a Windows SDK.
  # Use --quiet/--wait for CI. Remove --quiet if you want interactive debugging.
  & $vsInstaller modify --installPath "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools" `
    --add Microsoft.VisualStudio.Workload.VCTools `
    --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    --add Microsoft.VisualStudio.Component.Windows10SDK.19041 `
    --includeRecommended --passive --norestart --wait

  $vsInstall = & vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath

  if (-not $vsInstall) {
    throw "Visual Studio Build Tools with MSVC should be installed"
  }

  Write-Host "Installed Visual Studio Build Tools at: $vsInstall" -ForegroundColor Green
}

function Ensure-MsvcEnv {
  if (Detect-InNativeTools) {
    Write-Host "MSVC environment already initialized." -ForegroundColor Green
    return
  }

  Ensure-VSBuildTools

  $vsInstall = & vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath

  $vsDevCmd  = Join-Path $vsInstall "Common7\Tools\VsDevCmd.bat"
  $vcVarsAll = Join-Path $vsInstall "VC\Auxiliary\Build\vcvarsall.bat"

  if (Test-Path $vsDevCmd)      { Import-EnvFromCmd "`"$vsDevCmd`" -arch=x64" }
  elseif (Test-Path $vcVarsAll) { Import-EnvFromCmd "`"$vcVarsAll`" x64" }
  else                          { throw "VsDevCmd.bat or vcvarsall.bat should exist under $vsInstall" }

  foreach ($exe in @("cl.exe","link.exe","lib.exe")) {
    if (-not (Get-Command $exe -ErrorAction SilentlyContinue)) {
      throw "$exe should be available after MSVC env init"
    }
  }

  Write-Host "MSVC toolchain ready (cl/link/lib present)." -ForegroundColor Green
}

function Ensure-Scoop {
  if (Get-Command scoop -ErrorAction SilentlyContinue) {
    Write-Host "Scoop found." -ForegroundColor Green
    return
  }
  Write-Host "Installing Scoop..." -ForegroundColor Cyan
  try {
    Set-ExecutionPolicy -Scope CurrentUser RemoteSigned -Force -ErrorAction Stop
  } catch {
    Write-Host "Execution policy already enforced at higher scope, continuing..." -ForegroundColor Yellow
  }
  Invoke-RestMethod https://get.scoop.sh | Invoke-Expression
  if (-not (Get-Command scoop -ErrorAction SilentlyContinue)) {
    throw "Scoop installation failed"
  }
}

function Ensure-Git {
  if (Get-Command git -ErrorAction SilentlyContinue) {
    Write-Host "Git found." -ForegroundColor Green
    return
  }
  Write-Host "Ensuring Scoop 'main' bucket..." -ForegroundColor Cyan
  scoop bucket add main https://github.com/ScoopInstaller/Main
  Write-Host "Installing git via Scoop..." -ForegroundColor Cyan
  scoop install git
  if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    throw "Git installation failed"
  }
}

function Ensure-Bash {
  $gitRoot = $null
  try {
    $gitRoot = scoop prefix git 2>$null
  } catch {
    $gitRoot = $null
  }
  if (-not $gitRoot) {
    # Fallback if not installed via Scoop
    $candidate = "${env:ProgramFiles}\Git"
    if (Test-Path $candidate) { $gitRoot = $candidate }
  }
  if (-not $gitRoot) {
    Write-Host "Git root not found; cannot create bash shim." -ForegroundColor Yellow
    return
  }
  $bashExe = Join-Path $gitRoot "usr\bin\bash.exe"
  if (-not (Test-Path $bashExe)) {
    Write-Host "bash.exe not found under Git installation; skipping bash shim." -ForegroundColor Yellow
    return
  }
  Write-Host "Ensuring 'bash' shim..." -ForegroundColor Cyan
  scoop shim add bash $bashExe | Out-Null
}

function Export-CIPaths {
  # Only relevant in GitHub Actions
  if ($env:GITHUB_ACTIONS -ne "true") {
    return
  }

  $scoopRoot = $null
  if (Get-Command scoop -ErrorAction SilentlyContinue) {
    try {
      $scoopRoot = (& scoop config rootPath) -join ""
      if ([string]::IsNullOrWhiteSpace($scoopRoot)) { $scoopRoot = $null }
    } catch { $scoopRoot = $null }
  }
  if (-not $scoopRoot) {
    $candidate = Join-Path $env:USERPROFILE "scoop"
    if (Test-Path $candidate) { $scoopRoot = $candidate }
  }
  if (-not $scoopRoot) {
    throw "Scoop root path should be detected"
  }

  $shims = Join-Path $scoopRoot "shims"
  if (-not (Test-Path $shims)) {
    throw "Scoop shims directory should exist"
  }
  $shims | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
  $gitPrefix = $null
  try { $gitPrefix = (& scoop prefix git) -join "" } catch { $gitPrefix = $null }

  if ($gitPrefix) {
    $gitUsrBin = Join-Path $gitPrefix "usr\bin"
    if (Test-Path $gitUsrBin) {
      $gitUsrBin | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
    }
  }
  "SCOOP=$scoopRoot" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
  bash --version
}


Ensure-VSBuildTools
Ensure-Scoop
Ensure-Git

Write-Host "Adding Scoop buckets..." -ForegroundColor Cyan
scoop bucket add extras

# Tools (git already ensured above, keep it out of list)
$tools = @(
  "cmake",
  "ninja",
  "7zip"
)

# Skip python in GitHub Actions (we install it with actions/setup-python in the workflow)
if ($env:GITHUB_ACTIONS -eq "true") {
  Write-Host "GITHUB_ACTIONS=true: skipping Scoop Python install; use actions/setup-python." -ForegroundColor Yellow
} else {
  $tools += "python"
}

foreach ($tool in $tools) {
  if (scoop which $tool 2>$null) {
    Write-Host "$tool already installed" -ForegroundColor Green
  } else {
    Write-Host "Installing $tool..." -ForegroundColor Cyan
    scoop install $tool
  }
}

Ensure-Bash
Export-CIPaths

Write-Host "Scoop prerequisites installed." -ForegroundColor Green
