<#
.SYNOPSIS
    Builds the Zed Windows EXE installer package.

.DESCRIPTION
    This script automates building Zed and packaging it into a standalone Windows installer (.exe)
    using Inno Setup.

.PARAMETER Channel
    Release channel: 'dev' (default), 'nightly', 'preview', or 'stable'.

.PARAMETER Architecture
    Target architecture: 'x86_64' (default) or 'aarch64'.

.PARAMETER SkipBuild
    Skip cargo build steps and package existing binaries in target/release.

.PARAMETER SkipLicenses
    Skip generating licenses with cargo-about.

.PARAMETER SkipRemoteServer
    Skip building and packaging remote_server.

.PARAMETER Install
    Automatically launch the generated installer when compilation finishes.

.PARAMETER OpenFolder
    Open Windows File Explorer showing the generated installer .exe.

.EXAMPLE
    .\build-installer.ps1
    Builds the installer for the current channel.

.EXAMPLE
    .\build-installer.ps1 -Channel dev -Install
    Builds and immediately launches the installer.

.EXAMPLE
    .\build-installer.ps1 -SkipBuild -OpenFolder
    Packages existing target/release binaries without rebuilding from source.
#>

[CmdletBinding()]
Param(
    [Parameter()][Alias('i')][switch]$Install,
    [Parameter()][Alias('h')][switch]$Help,
    [Parameter()][Alias('a')][ValidateSet("x86_64", "aarch64")][string]$Architecture,
    [Parameter()][Alias('c')][ValidateSet("dev", "nightly", "preview", "stable")][string]$Channel,
    [Parameter()][switch]$SkipBuild,
    [Parameter()][switch]$SkipLicenses,
    [Parameter()][switch]$SkipRemoteServer,
    [Parameter()][Alias('o')][switch]$OpenFolder
)

$ErrorActionPreference = 'Stop'
$scriptPath = Join-Path $PSScriptRoot "script\bundle-windows.ps1"

if (-not (Test-Path $scriptPath)) {
    Write-Error "Could not find bundle script at: $scriptPath"
    exit 1
}

$params = @{}
if ($Install) { $params['Install'] = $true }
if ($Help) { $params['Help'] = $true }
if ($Architecture) { $params['Architecture'] = $Architecture }
if ($Channel) { $params['Channel'] = $Channel }
if ($SkipBuild) { $params['SkipBuild'] = $true }
if ($SkipLicenses) { $params['SkipLicenses'] = $true }
if ($SkipRemoteServer) { $params['SkipRemoteServer'] = $true }
if ($OpenFolder) { $params['OpenFolder'] = $true }

Push-Location $PSScriptRoot
try {
    & $scriptPath @params
} finally {
    Pop-Location
}
