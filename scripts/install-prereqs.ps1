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
    Set-ExecutionPolicy -Scope CurrentUser RemoteSigned -Force
    Invoke-RestMethod https://get.scoop.sh | Invoke-Expression

    if (-not (Get-Command scoop -ErrorAction SilentlyContinue)) {
        throw "Scoop installation failed"
    }
}

Ensure-Scoop

# Core buckets
scoop bucket add main
scoop bucket add extras

# Tools
$tools = @(
    "cmake",
    "ninja",
    "git",
    "git-bash",
    "7zip",
    "python"
)

foreach ($tool in $tools) {
    if (scoop which $tool 2>$null) {
        Write-Host "$tool already installed" -ForegroundColor Green
    } else {
        Write-Host "Installing $tool..." -ForegroundColor Cyan
        scoop install $tool
    }
}

Write-Host "Scoop prerequisites installed." -ForegroundColor Green
