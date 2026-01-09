<#
.SYNOPSIS
  Install prerequisites to build LLVM and generate bindings.

.USAGE
  pwsh -File install-prereqs.ps1
#>

$ErrorActionPreference = "Stop"

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

function Ensure-Git-Bootstrap {
    if (Get-Command git -ErrorAction SilentlyContinue) {
        Write-Host "Git found." -ForegroundColor Green
        return
    }

    # Scoop bucket operations require git. Bootstrap with winget if needed.
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        Write-Host "Bootstrapping Git via winget..." -ForegroundColor Cyan
        winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements
    }

    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        throw "Git is required but could not be installed (winget missing or failed)."
    }
}

function Ensure-Bash-Shim {
    $scoopRoot = (scoop prefix git 2>$null)
    if (-not $scoopRoot) {
        # If git was bootstrapped by winget, try common install path
        $candidate = "${env:ProgramFiles}\Git"
        if (Test-Path $candidate) { $scoopRoot = $candidate }
    }

    $bashExe = Join-Path $scoopRoot "usr\bin\bash.exe"
    if (-not (Test-Path $bashExe)) {
        Write-Host "bash.exe not found under Git installation; skipping bash shim." -ForegroundColor Yellow
        return
    }
    $shimDir = Join-Path $env:SCOOP "shims"
    if (-not (Test-Path $shimDir)) {
        New-Item -ItemType Directory -Path $shimDir | Out-Null
    }
    Write-Host "Ensuring 'bash' shim..." -ForegroundColor Cyan
    scoop shim add bash $bashExe | Out-Null
}

Ensure-Scoop
Ensure-Git-Bootstrap

# Core buckets
scoop bucket add main
scoop bucket add extras

# Tools
$tools = @(
    "cmake",
    "ninja",
    "git",
    "git-bash",
    "7zip"
)

# Skip python in CI, we install it with actions/setup-python in the workflow
$installPython = -not [bool]$env:CI
if ($installPython) {
    $tools += "python"
} else {
    Write-Host "CI detected (GITHUB_ACTIONS=1): skipping Scoop Python install; use actions/setup-python." -ForegroundColor Yellow
}

foreach ($tool in $tools) {
    if (scoop which $tool 2>$null) {
        Write-Host "$tool already installed" -ForegroundColor Green
    } else {
        Write-Host "Installing $tool..." -ForegroundColor Cyan
        scoop install $tool
    }
}

Ensure-Bash-Shim

Write-Host "Scoop prerequisites installed." -ForegroundColor Green
