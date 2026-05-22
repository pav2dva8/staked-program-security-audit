$ErrorActionPreference = "Stop"

$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
  [Security.Principal.WindowsBuiltInRole]::Administrator
)

if (-not $isAdmin) {
  Write-Error "Run this script from an elevated PowerShell prompt."
}

$repoRoot = Split-Path -Parent $PSScriptRoot
$logDir = Join-Path $repoRoot "logs"
New-Item -ItemType Directory -Path $logDir -Force | Out-Null

$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$logPath = Join-Path $logDir "docker-windows-repair-$timestamp.log"
Start-Transcript -Path $logPath -Force | Out-Null

try {
  Write-Host "Repairing Docker Desktop prerequisites for the WSL engine..."

  $features = @(
    "Microsoft-Windows-Subsystem-Linux",
    "VirtualMachinePlatform"
  )

  $restartNeeded = $false

  foreach ($featureName in $features) {
    $feature = Get-WindowsOptionalFeature -Online -FeatureName $featureName
    Write-Host "$featureName state: $($feature.State)"

    if ($feature.State -ne "Enabled") {
      $result = Enable-WindowsOptionalFeature -Online -FeatureName $featureName -All -NoRestart
      $restartNeeded = $restartNeeded -or [bool] $result.RestartNeeded
    }
  }

  $hyperV = Get-WindowsOptionalFeature -Online -FeatureName "Microsoft-Hyper-V-All" -ErrorAction SilentlyContinue
  if ($hyperV) {
    Write-Host "Microsoft-Hyper-V-All state: $($hyperV.State)"
    Write-Host "Docker Desktop's WSL engine does not require full Hyper-V; leaving this feature unchanged."
  }

  Write-Host "Setting hypervisorlaunchtype to auto..."
  bcdedit /set hypervisorlaunchtype auto | Out-Host

  Write-Host "Setting WSL default version to 2..."
  wsl --set-default-version 2 | Out-Host

  foreach ($serviceName in @("vmcompute", "hns", "LxssManager", "com.docker.service")) {
    $service = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
    if ($service) {
      Write-Host "Service $serviceName state: $($service.Status)"
      if ($service.Status -ne "Running") {
        Start-Service -Name $serviceName -ErrorAction Continue
      }
    }
  }

  if ($restartNeeded) {
    Write-Host "A Windows restart is required before Docker Desktop can use WSL2."
  } else {
    Write-Host "No feature install restart was reported. If Docker still fails, restart Windows anyway."
  }

  Write-Host "Log written to $logPath"
} finally {
  Stop-Transcript | Out-Null
}
