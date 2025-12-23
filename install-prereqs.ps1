<#
.SYNOPSIS
  Install prerequisites to build LLVM and generate bindings.

.USAGE
  pwsh -File install-prereqs.ps1
#>

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

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

function Choco-Install([string[]]$packages) {
  $pkgStr = ($packages -join ' ')
  Exec "choco install $pkgStr --yes --no-progress" "Chocolatey install failed for: $pkgStr"
}

function Ensure-Tools {
  Write-Section "Installing prerequisites via Chocolatey"

  # Minimal set for your LLVM/MLIR build + packaging flow on Windows:
  # - cmake, ninja: build system
  # - 7zip: .xz packaging
  # - git: clone llvm-project
  # - vswhere: locate VS install
  $toInstall = @("cmake","ninja","7zip","git","vswhere")
  Choco-Install $toInstall

  # Refresh PATH
  $env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" +
              [System.Environment]::GetEnvironmentVariable("Path","User")

  foreach ($tool in @("cmake","ninja","7z","git","vswhere")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
      throw "Missing tool after install: $tool (check PATH / restart shell)"
    }
  }

  Write-Host "Prerequisites installed and available on PATH." -ForegroundColor Green
}

Write-Section "Chocolatey setup"
Ensure-Choco

Write-Section "Prerequisites"
Ensure-Tools

Write-Host "`nDone." -ForegroundColor Green
