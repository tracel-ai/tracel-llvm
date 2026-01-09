<#
.SYNOPSIS
  Install Windows prerequisites for LLVM build (Chocolatey-based).

.USAGE
  pwsh -File install-prereqs.ps1
#>

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

function Ensure-Choco {
  if (Get-Command choco -ErrorAction SilentlyContinue) {
    Write-Host "Chocolatey found." -ForegroundColor Green
    return
  }

  Write-Host "Installing Chocolatey..." -ForegroundColor Cyan
  Set-ExecutionPolicy Bypass -Scope Process -Force | Out-Null
  [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12
  $script = (New-Object Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1')
  Invoke-Expression $script

  if (-not (Get-Command choco -ErrorAction SilentlyContinue)) {
    throw "Chocolatey installation failed"
  }
}

function Choco-Install([string[]]$packages) {
  Write-Host "Installing via Chocolatey: $($packages -join ', ')" -ForegroundColor Cyan
  & choco install @($packages) --yes --no-progress
  if ($LASTEXITCODE -ne 0) {
    throw "Chocolatey install failed (exit $LASTEXITCODE)"
  }
}

function Ensure-Basic-Tools {
  Choco-Install @(
    "git",
    "python",
    "cmake",
    "ninja",
    "7zip",
    "vswhere"
  )
}

function Ensure-VSBuildTools {
  $vsInstall = & vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath

  if ($vsInstall) {
    Write-Host "Found Visual Studio Build Tools at: $vsInstall" -ForegroundColor Green
    return
  }

  Write-Host "Installing Visual Studio 2022 Build Tools..." -ForegroundColor Yellow
  Choco-Install @(
    "visualstudio2022buildtools",
    "visualstudio2022-workload-vctools"
  )

  $vsInstall = & vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath

  if (-not $vsInstall) {
    throw "Visual Studio Build Tools with MSVC should be installed"
  }
}

function Ensure-Rust {
  if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Write-Host "Installing rustup via Chocolatey..." -ForegroundColor Yellow
    Choco-Install @("rustup.install")
  } else {
    Write-Host "rustup found." -ForegroundColor Green
  }
}

# ---------------- main ----------------

Ensure-Choco
Ensure-Rust
Ensure-Basic-Tools
Ensure-VSBuildTools

Write-Host "Windows prerequisites installed." -ForegroundColor Green
