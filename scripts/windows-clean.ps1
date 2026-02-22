$ErrorActionPreference = "Stop"

function Invoke-IgnoringErrors {
    param([scriptblock]$Action)
    try {
        & $Action
    } catch {
    }
}

function Ensure-Elevated {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        $args = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "`"$PSCommandPath`"")
        try {
            Start-Process -FilePath "powershell.exe" -ArgumentList $args -Verb RunAs -ErrorAction Stop
        } catch {
            Write-Error "Administrator privileges are required to uninstall and remove Program Files entries."
        }
        exit 1
    }
}

function Stop-FlowSTTProcesses {
    $names = @("flowstt-app", "flowstt", "flowstt-service")
    foreach ($name in $names) {
        Invoke-IgnoringErrors { Get-Process -Name $name -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue }
    }
}

function Get-FlowSTTUninstallEntries {
    $roots = @(
        "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"
    )

    $entries = @()
    foreach ($root in $roots) {
        if (-not (Test-Path $root)) {
            continue
        }
        $entries += Get-ChildItem -Path $root -ErrorAction SilentlyContinue |
            ForEach-Object { Get-ItemProperty -Path $_.PSPath -ErrorAction SilentlyContinue } |
            Where-Object { $_.DisplayName -and $_.DisplayName -match "(?i)flowstt" }
    }
    $entries
}

function Invoke-CommandLine {
    param([string]$CommandLine)

    if (-not $CommandLine) {
        return
    }

    $exe = $null
    $args = ""
    if ($CommandLine -match '^\s*"([^"]+)"\s*(.*)$') {
        $exe = $matches[1]
        $args = $matches[2]
    } else {
        $parts = $CommandLine.Split(" ", 2)
        $exe = $parts[0]
        if ($parts.Length -gt 1) {
            $args = $parts[1]
        }
    }

    if (-not $exe) {
        return
    }

    if ($exe -match "(?i)uninstall\.exe" -and $args -notmatch "(?i)\s/\s*s\b") {
        $args = ($args + " /S").Trim()
    }

    Start-Process -FilePath $exe -ArgumentList $args -Wait -NoNewWindow -ErrorAction SilentlyContinue
}

function Get-MsiUninstallArgs {
    param([string]$Args)

    $guidMatch = [regex]::Match($Args, "\{[0-9A-Fa-f-]+\}")
    if ($Args -match "(?i)\s/(i|install)\b") {
        $Args = [regex]::Replace($Args, "(?i)\s/(i|install)\b", " /x")
    } elseif ($guidMatch.Success) {
        $Args = "/x $($guidMatch.Value)"
    }

    $Args = [regex]::Replace($Args, "(?i)\s/\s*q[a-z]*\b", "")
    $Args = [regex]::Replace($Args, "(?i)\s/\s*quiet\b", "")
    $Args = ($Args + " /qn /norestart").Trim()
    $Args
}

function Get-MsiProductCode {
    param($Entry)

    if ($Entry.PSChildName -and $Entry.PSChildName -match "^\{[0-9A-Fa-f-]+\}$") {
        return $Entry.PSChildName
    }

    $command = $Entry.UninstallString
    if (-not $command) {
        $command = $Entry.QuietUninstallString
    }

    if ($command) {
        $match = [regex]::Match($command, "\{[0-9A-Fa-f-]+\}")
        if ($match.Success) {
            return $match.Value
        }
    }

    return $null
}

function Invoke-UninstallEntry {
    param($Entry)

    $command = $Entry.QuietUninstallString
    if (-not $command) {
        $command = $Entry.UninstallString
    }

    if (-not $command) {
        return
    }

    if ($command -match "(?i)msiexec") {
        $productCode = Get-MsiProductCode -Entry $Entry
        $exe = "msiexec.exe"
        if ($productCode) {
            $args = "/x $productCode /qn /norestart"
        } else {
            $args = $command -replace "(?i)^\s*msiexec(\.exe)?\s*", ""
            $args = Get-MsiUninstallArgs -Args $args
        }
        $process = Start-Process -FilePath $exe -ArgumentList $args -Wait -NoNewWindow -PassThru -ErrorAction SilentlyContinue
        if ($process -and $process.ExitCode -ne 0 -and $process.ExitCode -ne 3010 -and $productCode) {
            Start-Process -FilePath $exe -ArgumentList "/x $productCode /qn /norestart" -Wait -NoNewWindow -ErrorAction SilentlyContinue
        }
    } else {
        Invoke-CommandLine -CommandLine $command
    }
}

function Remove-IfExists {
    param([string]$Path)
    if ([string]::IsNullOrWhiteSpace($Path)) {
        return
    }
    if (Test-Path -Path $Path) {
        Remove-Item -Path $Path -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Remove-ConsentEntries {
    $roots = @(
        "HKCU:\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone",
        "HKCU:\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\NonPackaged"
    )

    foreach ($root in $roots) {
        if (-not (Test-Path $root)) {
            continue
        }
        Get-ChildItem -Path $root -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -match "(?i)flowstt" } |
            ForEach-Object { Remove-Item -Path $_.PSPath -Recurse -Force -ErrorAction SilentlyContinue }
    }
}

Ensure-Elevated
Stop-FlowSTTProcesses

Invoke-IgnoringErrors {
    Get-AppxPackage -Name "io.flowstt*" -ErrorAction SilentlyContinue | Remove-AppxPackage -ErrorAction SilentlyContinue
    Get-AppxPackage -Name "FlowSTT*" -ErrorAction SilentlyContinue | Remove-AppxPackage -ErrorAction SilentlyContinue
}

Invoke-IgnoringErrors {
    $entries = Get-FlowSTTUninstallEntries
    foreach ($entry in $entries) {
        Invoke-UninstallEntry -Entry $entry
    }
}

Remove-ConsentEntries

function Join-IfRoot {
    param([string]$Root, [string]$Child)
    if ([string]::IsNullOrWhiteSpace($Root)) {
        return $null
    }
    Join-Path -Path $Root -ChildPath $Child
}

$paths = @()
$paths += Join-IfRoot -Root ([string]$env:LOCALAPPDATA) -Child "Programs\FlowSTT"
$paths += Join-IfRoot -Root ([string]$env:APPDATA) -Child "flowstt"
$paths += Join-IfRoot -Root ([string]$env:APPDATA) -Child "io.flowstt"
$paths += Join-IfRoot -Root ([string]$env:APPDATA) -Child "com.keath.flowstt"
$paths += Join-IfRoot -Root ([string]$env:LOCALAPPDATA) -Child "flowstt"
$paths += Join-IfRoot -Root ([string]$env:LOCALAPPDATA) -Child "flowstt-app"
$paths += Join-IfRoot -Root ([string]$env:LOCALAPPDATA) -Child "io.flowstt"
$paths += Join-IfRoot -Root ([string]$env:LOCALAPPDATA) -Child "com.keath.flowstt"
$paths += Join-IfRoot -Root ([string]$env:LOCALAPPDATA) -Child "whisper"
$paths += Join-IfRoot -Root ([string]$env:TEMP) -Child "flowstt-logs"

if ($env:ProgramFiles) {
    $paths += Join-IfRoot -Root ([string]$env:ProgramFiles) -Child "FlowSTT"
    $paths += Join-IfRoot -Root ([string]$env:ProgramFiles) -Child "FlowSTT App"
}

if ($env:ProgramW6432) {
    $paths += Join-IfRoot -Root ([string]$env:ProgramW6432) -Child "FlowSTT"
    $paths += Join-IfRoot -Root ([string]$env:ProgramW6432) -Child "FlowSTT App"
}

if ($env:ProgramFiles -and $env:ProgramFiles -match "Program Files" -and $env:ProgramFiles -ne "") {
    if ($env:ProgramFiles -notmatch "x86" -and $env:ProgramFiles -ne "") {
        $pf86 = ${env:ProgramFiles(x86)}
        if ($pf86) {
            $paths += Join-IfRoot -Root ([string]$pf86) -Child "FlowSTT"
        }
    }
}

foreach ($path in $paths) {
    Remove-IfExists -Path $path
}

$packageRoot = Join-IfRoot -Root ([string]$env:LOCALAPPDATA) -Child "Packages"
if (Test-Path $packageRoot) {
    Get-ChildItem -Path $packageRoot -Filter "io.flowstt*" -ErrorAction SilentlyContinue |
        ForEach-Object { Remove-Item -Path $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
    Get-ChildItem -Path $packageRoot -Filter "FlowSTT*" -ErrorAction SilentlyContinue |
        ForEach-Object { Remove-Item -Path $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
}

$shortcuts = @()
$shortcuts += Join-IfRoot -Root ([string]$env:APPDATA) -Child "Microsoft\Windows\Start Menu\Programs\FlowSTT.lnk"
$shortcuts += Join-IfRoot -Root ([string]$env:APPDATA) -Child "Microsoft\Windows\Start Menu\Programs\FlowSTT\FlowSTT.lnk"
$shortcuts += Join-IfRoot -Root ([string]$env:PUBLIC) -Child "Desktop\FlowSTT.lnk"
$shortcuts += Join-IfRoot -Root ([string]$env:USERPROFILE) -Child "Desktop\FlowSTT.lnk"

foreach ($shortcut in $shortcuts) {
    Remove-IfExists -Path $shortcut
}

$regPaths = @(
    "HKCU:\Software\FlowSTT",
    "HKCU:\Software\flowstt",
    "HKCU:\Software\io.flowstt",
    "HKCU:\Software\com.keath.flowstt"
)

foreach ($regPath in $regPaths) {
    Invoke-IgnoringErrors { Remove-Item -Path $regPath -Recurse -Force -ErrorAction SilentlyContinue }
}
