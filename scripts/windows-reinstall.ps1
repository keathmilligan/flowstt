$ErrorActionPreference = "Stop"

function Ensure-Elevated {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        $args = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "`"$PSCommandPath`"")
        try {
            Start-Process -FilePath "powershell.exe" -ArgumentList $args -Verb RunAs -ErrorAction Stop
        } catch {
            Write-Error "Administrator privileges are required to reinstall FlowSTT."
        }
        exit 1
    }
}

function Start-Installer {
    param(
        [string]$FilePath,
        [string]$ArgumentList,
        [int[]]$SuccessExitCodes = @(0, 3010)
    )

    $process = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -Wait -NoNewWindow -PassThru
    if (-not $process) {
        throw "Failed to launch installer: $FilePath"
    }
    if ($SuccessExitCodes -notcontains $process.ExitCode) {
        throw "Installer failed with exit code $($process.ExitCode): $FilePath"
    }
}

Ensure-Elevated

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")

& (Join-Path $scriptDir "windows-clean.ps1")

$bundleRoot = Join-Path $repoRoot "target\release\bundle"

$msi = Get-ChildItem -Path (Join-Path $bundleRoot "msi") -Filter "*.msi" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

$exe = Get-ChildItem -Path (Join-Path $bundleRoot "nsis") -Filter "*.exe" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if ($msi) {
    Start-Installer -FilePath "msiexec.exe" -ArgumentList "/i `"$($msi.FullName)`" /qn /norestart"
    exit 0
}

if ($exe) {
    Start-Installer -FilePath $exe.FullName -ArgumentList "/S" -SuccessExitCodes @(0)
    exit 0
}

throw "No Windows installer found under target\\release\\bundle. Build a release bundle first."
