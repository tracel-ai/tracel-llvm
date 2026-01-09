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


Ensure-Scoop
Ensure-Git

Write-Host "Adding Scoop buckets..." -ForegroundColor Cyan
scoop bucket add extras

# Tools (git already ensured above, keep it out of list)
$tools = @(
    "cmake",
    "ninja",
    "git-bash",
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
