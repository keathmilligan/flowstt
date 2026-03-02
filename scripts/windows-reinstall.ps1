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

Ensure-Elevated

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

& (Join-Path $scriptDir "windows-clean.ps1")
& (Join-Path $scriptDir "windows-install.ps1")
