[CmdletBinding()]
Param(
    [Parameter()][Alias('i')][switch]$Install,
    [Parameter()][Alias('h')][switch]$Help,
    [Parameter()][Alias('a')][ValidateSet("x86_64", "aarch64")][string]$Architecture,
    [Parameter()][Alias('c')][ValidateSet("dev", "nightly", "preview", "stable")][string]$Channel,
    [Parameter()][switch]$SkipBuild,
    [Parameter()][switch]$SkipLicenses,
    [Parameter()][switch]$SkipRemoteServer,
    [Parameter()][Alias('o')][switch]$OpenFolder,
    [Parameter()][string]$Name
)

. "$PSScriptRoot/lib/workspace.ps1"

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

$buildSuccess = $false
$canCodeSign = $false
$finalInstallerPath = ""

if ($Help) {
    Write-Host @"
============================================================
 Zed Windows Installer Build Script
============================================================
Usage: .\script\bundle-windows.ps1 [options]
   or: .\build-installer.ps1 [options]

Options:
  -Architecture, -a <x86_64|aarch64>  Target CPU architecture (default: system arch)
  -Channel, -c <dev|nightly|preview|stable>
                                      Release channel (default: from crates/zed/RELEASE_CHANNEL)
  -SkipBuild                          Skip 'cargo build' and use existing target/release binaries
  -SkipLicenses                       Skip 'cargo-about' license generation
  -SkipRemoteServer                   Skip building the remote_server archive
  -Install, -i                        Automatically launch the installer after successful build
  -OpenFolder, -o                     Open output directory in Windows Explorer when done
  -Help, -h                           Show this help message

Examples:
  .\build-installer.ps1
  .\build-installer.ps1 -Channel dev -Install
  .\build-installer.ps1 -SkipBuild -OpenFolder
"@
    exit 0
}

$OSArchitecture = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
    "X64" { "x86_64" }
    "Arm64" { "aarch64" }
    default { throw "Unsupported architecture: $([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture)" }
}

$Architecture = if ($Architecture) {
    $Architecture
} else {
    $OSArchitecture
}

$CargoOutDir = "./target/$Architecture-pc-windows-msvc/release"
$target = "$Architecture-pc-windows-msvc"

function Get-VSArch {
    param([string]$Arch)
    switch ($Arch) {
        "x86_64" { "amd64" }
        "aarch64" { "arm64" }
    }
}

function Setup-VisualStudioEnvironment {
    try {
        $vswherePath = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
        if (-not (Test-Path $vswherePath)) {
            Write-Host "ℹ️ vswhere not found at default location, relying on existing environment." -ForegroundColor Gray
            return
        }
        $vsInstallPath = & $vswherePath -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ([string]::IsNullOrWhiteSpace($vsInstallPath)) {
            Write-Host "ℹ️ Visual Studio C++ tools not located via vswhere, relying on existing environment." -ForegroundColor Gray
            return
        }
        $vsDevShellPath = Join-Path $vsInstallPath "Common7\Tools\Launch-VsDevShell.ps1"
        if (Test-Path $vsDevShellPath) {
            Push-Location
            & $vsDevShellPath -Arch (Get-VSArch -Arch $Architecture) -HostArch (Get-VSArch -Arch $OSArchitecture) | Out-Null
            Pop-Location
        }
    } catch {
        Write-Host "⚠️ Warning initializing VS DevShell: $_" -ForegroundColor Yellow
    }
}

Setup-VisualStudioEnvironment

# Determine release channel
if (-not $Channel) {
    if (Test-Path "$PSScriptRoot/../crates/zed/RELEASE_CHANNEL") {
        $Channel = (Get-Content "$PSScriptRoot/../crates/zed/RELEASE_CHANNEL").Trim()
    } else {
        $Channel = "dev"
    }
}
$channel = $Channel
$env:ZED_RELEASE_CHANNEL = $channel
$env:RELEASE_CHANNEL = $channel

function CheckEnvironmentVariables {
    if (-not $env:CI) {
        return
    }

    $requiredVars = @('ZED_WORKSPACE', 'RELEASE_VERSION', 'ZED_RELEASE_CHANNEL')
    foreach ($var in $requiredVars) {
        if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($var))) {
            Write-Error "$var is not set"
            exit 1
        }
    }

    $signingVars = @(
        'AZURE_TENANT_ID', 'AZURE_CLIENT_ID', 'AZURE_CLIENT_SECRET',
        'ACCOUNT_NAME', 'CERT_PROFILE_NAME', 'ENDPOINT',
        'FILE_DIGEST', 'TIMESTAMP_DIGEST', 'TIMESTAMP_SERVER'
    )

    $missingVars = @($signingVars | Where-Object { [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($_)) })
    if ($missingVars.Count -eq 0) {
        $script:canCodeSign = $true
    } else {
        Write-Host "====== WARNING ======" -ForegroundColor Yellow
        Write-Host "One or more of the following variables are missing: $($missingVars -join ', ')" -ForegroundColor Yellow
        Write-Host "This bundle will not be code signed" -ForegroundColor Yellow
        Write-Host "=====================" -ForegroundColor Yellow
    }
}

function Get-InnoSetupPath {
    $inPath = Get-Command "ISCC.exe" -ErrorAction SilentlyContinue
    if ($inPath) {
        return $inPath.Source
    }
    $candidates = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
        "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
        "C:\Program Files\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    )
    foreach ($path in $candidates) {
        if ($path -and (Test-Path $path)) {
            return $path
        }
    }
    return $null
}

function Get-MakeAppxPath {
    param([string]$Arch = "x64")
    $inPath = Get-Command "makeappx.exe" -ErrorAction SilentlyContinue
    if ($inPath) {
        return $inPath.Source
    }
    $kitsRoot = "C:\Program Files (x86)\Windows Kits\10\bin"
    if (Test-Path $kitsRoot) {
        $versionDirs = Get-ChildItem -Path $kitsRoot -Directory -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -match '^\d+\.\d+\.\d+\.\d+$' } |
            Sort-Object { [version]$_.Name } -Descending
        foreach ($dir in $versionDirs) {
            $makeappx = Join-Path $dir.FullName "$Arch\makeappx.exe"
            if (Test-Path $makeappx) {
                return $makeappx
            }
            $makeappxX64 = Join-Path $dir.FullName "x64\makeappx.exe"
            if (Test-Path $makeappxX64) {
                return $makeappxX64
            }
        }
    }
    return $null
}

function PrepareForBundle {
    Write-Host "📁 Preparing staging directory: $innoDir..." -ForegroundColor Cyan
    if (Test-Path "$innoDir") {
        Remove-Item -Path "$innoDir" -Recurse -Force
    }
    New-Item -Path "$innoDir" -ItemType Directory -Force | Out-Null
    Copy-Item -Path "$env:ZED_WORKSPACE\crates\zed\resources\windows\*" -Destination "$innoDir" -Recurse -Force
    New-Item -Path "$innoDir\make_appx" -ItemType Directory -Force | Out-Null
    New-Item -Path "$innoDir\appx" -ItemType Directory -Force | Out-Null
    New-Item -Path "$innoDir\bin" -ItemType Directory -Force | Out-Null
    New-Item -Path "$innoDir\tools" -ItemType Directory -Force | Out-Null

    rustup target add $target
}

function GenerateLicenses {
    if ($SkipLicenses) {
        Write-Host "⏩ Skipping license generation (-SkipLicenses)" -ForegroundColor Yellow
        if (-not (Test-Path "$env:ZED_WORKSPACE\assets\licenses.md")) {
            New-Item -Path "$env:ZED_WORKSPACE\assets\licenses.md" -ItemType File -Value "# Licenses" -Force | Out-Null
        }
        return
    }
    Write-Host "📜 Generating licenses..." -ForegroundColor Cyan
    . $PSScriptRoot/generate-licenses.ps1
}

function BuildZedAndItsFriends {
    if ($SkipBuild) {
        Write-Host "⏩ Skipping Cargo build as requested (-SkipBuild)" -ForegroundColor Yellow
        $requiredFiles = @(
            ".\$CargoOutDir\zed.exe",
            ".\$CargoOutDir\cli.exe",
            ".\$CargoOutDir\auto_update_helper.exe",
            ".\$CargoOutDir\explorer_command_injector.dll"
        )
        foreach ($file in $requiredFiles) {
            if (-not (Test-Path $file)) {
                throw "Required binary $file does not exist. Please build without -SkipBuild or compile release target first."
            }
        }
    } else {
        Write-Host "🔨 Building Zed components (channel: $channel, target: $target)..." -ForegroundColor Cyan
        cargo build --release --package zed --package cli --package auto_update_helper --target $target
        switch ($channel) {
            "stable" {
                cargo build --release --features stable --no-default-features --package explorer_command_injector --target $target
            }
            "preview" {
                cargo build --release --features preview --no-default-features --package explorer_command_injector --target $target
            }
            default {
                cargo build --release --package explorer_command_injector --target $target
            }
        }
    }

    Copy-Item -Path ".\$CargoOutDir\zed.exe" -Destination "$innoDir\Zed.exe" -Force
    Copy-Item -Path ".\$CargoOutDir\cli.exe" -Destination "$innoDir\cli.exe" -Force
    Copy-Item -Path ".\$CargoOutDir\auto_update_helper.exe" -Destination "$innoDir\auto_update_helper.exe" -Force
    Copy-Item -Path ".\$CargoOutDir\explorer_command_injector.dll" -Destination "$innoDir\zed_explorer_command_injector.dll" -Force
}

function BuildRemoteServer {
    if ($SkipRemoteServer) {
        Write-Host "⏩ Skipping remote_server build (-SkipRemoteServer)" -ForegroundColor Yellow
        return
    }

    if ($SkipBuild) {
        if (-not (Test-Path ".\$CargoOutDir\remote_server.exe")) {
            Write-Host "⏩ remote_server.exe not found, skipping remote server archive" -ForegroundColor Yellow
            return
        }
    } else {
        Write-Host "🔨 Building remote_server for $target..." -ForegroundColor Cyan
        cargo build --release --package remote_server --target $target
    }

    $remoteServerSrc = (Resolve-Path ".\$CargoOutDir\remote_server.exe").Path
    if ($canCodeSign) {
        Write-Host "🔐 Code signing remote_server.exe..." -ForegroundColor Cyan
        & "$innoDir\sign.ps1" $remoteServerSrc
    }

    $remoteServerDst = "$env:ZED_WORKSPACE\target\zed-remote-server-windows-$Architecture.zip"
    Write-Host "📦 Compressing remote_server to $remoteServerDst..." -ForegroundColor Cyan
    Compress-Archive -Path $remoteServerSrc -DestinationPath $remoteServerDst -Force
}

function ZipZedAndItsFriendsDebug {
    $items = @(
        ".\$CargoOutDir\zed.pdb",
        ".\$CargoOutDir\cli.pdb",
        ".\$CargoOutDir\auto_update_helper.pdb",
        ".\$CargoOutDir\explorer_command_injector.pdb",
        ".\$CargoOutDir\remote_server.pdb"
    ) | Where-Object { Test-Path $_ }

    if ($items.Count -gt 0) {
        Write-Host "📦 Creating debug symbols archive..." -ForegroundColor Cyan
        Compress-Archive -Path $items -DestinationPath ".\$CargoOutDir\zed-$env:RELEASE_VERSION-$env:ZED_RELEASE_CHANNEL.dbg.zip" -Force
    }
}

function UploadToSentry {
    if (-not (Get-Command "sentry-cli" -ErrorAction SilentlyContinue)) {
        Write-Output "sentry-cli not found. skipping sentry upload."
        return
    }
    if ([string]::IsNullOrWhiteSpace($env:SENTRY_AUTH_TOKEN)) {
        Write-Output "missing SENTRY_AUTH_TOKEN. skipping sentry upload."
        return
    }
    Write-Output "Uploading zed debug symbols to sentry..."
    for ($i = 1; $i -le 3; $i++) {
        try {
            sentry-cli debug-files upload --include-sources --wait -p zed -o zed-dev $CargoOutDir
            break
        } catch {
            Write-Output "Sentry upload attempt $i failed: $_"
            if ($i -eq 3) {
                Write-Output "All sentry upload attempts failed"
                throw
            }
            Start-Sleep -Seconds 2
        }
    }
}

function MakeAppx {
    Write-Host "📦 Packaging Explorer context menu AppX..." -ForegroundColor Cyan
    switch ($channel) {
        "stable" {
            $manifestFile = "$env:ZED_WORKSPACE\crates\explorer_command_injector\AppxManifest.xml"
        }
        "preview" {
            $manifestFile = "$env:ZED_WORKSPACE\crates\explorer_command_injector\AppxManifest-Preview.xml"
        }
        default {
            $manifestFile = "$env:ZED_WORKSPACE\crates\explorer_command_injector\AppxManifest-Nightly.xml"
        }
    }
    Copy-Item -Path "$manifestFile" -Destination "$innoDir\make_appx\AppxManifest.xml" -Force

    $makeAppxExe = Get-MakeAppxPath -Arch (Get-VSArch -Arch $Architecture)
    if (-not $makeAppxExe) {
        throw "makeappx.exe was not found. Ensure Windows 10/11 SDK is installed."
    }

    & $makeAppxExe pack /d "$innoDir\make_appx" /p "$innoDir\zed_explorer_command_injector.appx" /nv
}

function SignZedAndItsFriends {
    if (-not $canCodeSign) {
        return
    }

    Write-Host "🔐 Code signing executables..." -ForegroundColor Cyan
    $files = "$innoDir\Zed.exe,$innoDir\cli.exe,$innoDir\auto_update_helper.exe,$innoDir\zed_explorer_command_injector.dll,$innoDir\zed_explorer_command_injector.appx"
    & "$innoDir\sign.ps1" $files
}

function Get-AgsDllPath {
    $cand = "$env:ZED_WORKSPACE\target\AGS_SDK-6.3.0\ags_lib\lib\amd_ags_x64.dll"
    if (Test-Path $cand) {
        return (Resolve-Path $cand).Path
    }
    return $null
}

function Get-ConptyDllPath {
    param([string]$Arch)
    $subDir = if ($Arch -eq "aarch64") { "win-arm64" } else { "win-x64" }
    $cand = "$env:ZED_WORKSPACE\target\conpty\runtimes\$subDir\native\conpty.dll"
    if (Test-Path $cand) {
        return (Resolve-Path $cand).Path
    }
    return $null
}

function Get-OpenConsolePath {
    param([string]$Arch)
    $subDir = if ($Arch -eq "aarch64") { "arm64" } else { "x64" }
    $cand = "$env:ZED_WORKSPACE\target\conpty\build\native\runtimes\$subDir\OpenConsole.exe"
    if (Test-Path $cand) {
        return (Resolve-Path $cand).Path
    }
    return $null
}

function DownloadAMDGpuServices {
    $agsDll = Get-AgsDllPath
    if ($agsDll) {
        return
    }

    $zipPath = "$env:ZED_WORKSPACE\target\AGS_SDK_v6.3.0.zip"
    $extractedDir = "$env:ZED_WORKSPACE\target\AGS_SDK-6.3.0"

    if (-not (Test-Path "$env:ZED_WORKSPACE\target")) {
        New-Item -Path "$env:ZED_WORKSPACE\target" -ItemType Directory -Force | Out-Null
    }
    if (-not (Test-Path $zipPath)) {
        Write-Host "🌐 Downloading AMD AGS SDK v6.3.0..." -ForegroundColor Cyan
        $url = "https://codeload.github.com/GPUOpen-LibrariesAndSDKs/AGS_SDK/zip/refs/tags/v6.3.0"
        Invoke-WebRequest -Uri $url -OutFile $zipPath
    }
    Write-Host "📦 Extracting AMD AGS SDK..." -ForegroundColor Cyan
    Expand-Archive -Path $zipPath -DestinationPath "$env:ZED_WORKSPACE\target" -Force
}

function DownloadConpty {
    $conptyDll = Get-ConptyDllPath -Arch $Architecture
    $openConsole = Get-OpenConsolePath -Arch $Architecture
    if ($conptyDll -and $openConsole) {
        return
    }

    $nupkgPath = "$env:ZED_WORKSPACE\target\Microsoft.Windows.Console.ConPTY.1.23.251216003.nupkg"
    $extractedDir = "$env:ZED_WORKSPACE\target\conpty"

    if (-not (Test-Path "$env:ZED_WORKSPACE\target")) {
        New-Item -Path "$env:ZED_WORKSPACE\target" -ItemType Directory -Force | Out-Null
    }
    if (-not (Test-Path $nupkgPath)) {
        Write-Host "🌐 Downloading ConPTY 1.23..." -ForegroundColor Cyan
        $url = "https://github.com/microsoft/terminal/releases/download/v1.23.13503.0/Microsoft.Windows.Console.ConPTY.1.23.251216003.nupkg"
        Invoke-WebRequest -Uri $url -OutFile $nupkgPath
    }
    Write-Host "📦 Extracting ConPTY..." -ForegroundColor Cyan
    Expand-Archive -Path $nupkgPath -DestinationPath $extractedDir -Force
}

function CollectFiles {
    Write-Host "📋 Collecting installer payload files..." -ForegroundColor Cyan
    if (Test-Path "$innoDir\zed_explorer_command_injector.appx") {
        Move-Item -Path "$innoDir\zed_explorer_command_injector.appx" -Destination "$innoDir\appx\zed_explorer_command_injector.appx" -Force
    }
    if (Test-Path "$innoDir\zed_explorer_command_injector.dll") {
        Move-Item -Path "$innoDir\zed_explorer_command_injector.dll" -Destination "$innoDir\appx\zed_explorer_command_injector.dll" -Force
    }
    if (Test-Path "$innoDir\cli.exe") {
        Move-Item -Path "$innoDir\cli.exe" -Destination "$innoDir\bin\zed.exe" -Force
    }
    if (Test-Path "$innoDir\zed.sh") {
        Move-Item -Path "$innoDir\zed.sh" -Destination "$innoDir\bin\zed" -Force
    }
    if (Test-Path "$innoDir\auto_update_helper.exe") {
        Move-Item -Path "$innoDir\auto_update_helper.exe" -Destination "$innoDir\tools\auto_update_helper.exe" -Force
    }

    if ($Architecture -eq "aarch64") {
        New-Item -Type Directory -Path "$innoDir\arm64" -Force | Out-Null
        $openConsole = Get-OpenConsolePath -Arch "aarch64"
        if ($openConsole) {
            Copy-Item -Path $openConsole -Destination "$innoDir\arm64\OpenConsole.exe" -Force
        }
        $conptyDll = Get-ConptyDllPath -Arch "aarch64"
        if ($conptyDll) {
            Copy-Item -Path $conptyDll -Destination "$innoDir\conpty.dll" -Force
        }
    } else {
        New-Item -Type Directory -Path "$innoDir\x64" -Force | Out-Null
        New-Item -Type Directory -Path "$innoDir\arm64" -Force | Out-Null
        
        $agsDll = Get-AgsDllPath
        if ($agsDll) {
            Copy-Item -Path $agsDll -Destination "$innoDir\amd_ags_x64.dll" -Force
        }
        $openConsoleX64 = Get-OpenConsolePath -Arch "x86_64"
        if ($openConsoleX64) {
            Copy-Item -Path $openConsoleX64 -Destination "$innoDir\x64\OpenConsole.exe" -Force
        }
        $openConsoleArm64 = Get-OpenConsolePath -Arch "aarch64"
        if ($openConsoleArm64) {
            Copy-Item -Path $openConsoleArm64 -Destination "$innoDir\arm64\OpenConsole.exe" -Force
        }
        $conptyDllX64 = Get-ConptyDllPath -Arch "x86_64"
        if ($conptyDllX64) {
            Copy-Item -Path $conptyDllX64 -Destination "$innoDir\conpty.dll" -Force
        }
    }
}

function BuildInstaller {
    $issFilePath = "$innoDir\zed.iss"
    switch ($channel) {
        "stable" {
            $appId = "{{2DB0DA96-CA55-49BB-AF4F-64AF36A86712}"
            $appIconName = "app-icon"
            $appName = "Zed"
            $appDisplayName = "Zed"
            $appSetupName = "Zed-$Architecture"
            $appMutex = "Zed-Stable-Instance-Mutex"
            $appExeName = "Zed"
            $regValueName = "Zed"
            $appUserId = "ZedIndustries.Zed"
            $appShellNameShort = "Z&ed"
            $appAppxFullName = "ZedIndustries.Zed_1.0.0.0_neutral__japxn1gcva8rg"
        }
        "preview" {
            $appId = "{{F70E4811-D0E2-4D88-AC99-D63752799F95}"
            $appIconName = "app-icon-preview"
            $appName = "Zed Preview"
            $appDisplayName = "Zed Preview"
            $appSetupName = "Zed-$Architecture"
            $appMutex = "Zed-Preview-Instance-Mutex"
            $appExeName = "Zed"
            $regValueName = "ZedPreview"
            $appUserId = "ZedIndustries.Zed.Preview"
            $appShellNameShort = "Z&ed Preview"
            $appAppxFullName = "ZedIndustries.Zed.Preview_1.0.0.0_neutral__japxn1gcva8rg"
        }
        "nightly" {
            $appId = "{{1BDB21D3-14E7-433C-843C-9C97382B2FE0}"
            $appIconName = "app-icon-nightly"
            $appName = "Zed Nightly"
            $appDisplayName = "Zed Nightly"
            $appSetupName = "Zed-$Architecture"
            $appMutex = "Zed-Nightly-Instance-Mutex"
            $appExeName = "Zed"
            $regValueName = "ZedNightly"
            $appUserId = "ZedIndustries.Zed.Nightly"
            $appShellNameShort = "Z&ed Editor Nightly"
            $appAppxFullName = "ZedIndustries.Zed.Nightly_1.0.0.0_neutral__japxn1gcva8rg"
        }
        "dev" {
            $appId = "{{8357632E-24A4-4F32-BA97-E575B4D1FE5D}"
            $appIconName = "app-icon-dev"
            $appName = "Zed Dev"
            $appDisplayName = "Zed Dev"
            $appSetupName = "Zed-$Architecture"
            $appMutex = "Zed-Dev-Instance-Mutex"
            $appExeName = "Zed"
            $regValueName = "ZedDev"
            $appUserId = "ZedIndustries.Zed.Dev"
            $appShellNameShort = "Z&ed Dev"
            $appAppxFullName = "ZedIndustries.Zed.Dev_1.0.0.0_neutral__japxn1gcva8rg"
        }
        default {
            Write-Error "can't bundle installer for $channel."
            exit 1
        }
    }

    $innoSetupPath = Get-InnoSetupPath
    if (-not $innoSetupPath) {
        throw "Inno Setup 6 was not found. Please install ISCC.exe before building (run: 'winget install JRSoftware.InnoSetup' or download from https://jrsoftware.org/isdl.php)."
    }

    $definitions = @{
        "AppId"          = $appId
        "AppIconName"    = $appIconName
        "OutputDir"      = "$env:ZED_WORKSPACE\target"
        "AppSetupName"   = $appSetupName
        "AppName"        = $appName
        "AppDisplayName" = $appDisplayName
        "RegValueName"   = $regValueName
        "AppMutex"       = $appMutex
        "AppExeName"     = $appExeName
        "ResourcesDir"   = "$innoDir"
        "ShellNameShort" = $appShellNameShort
        "AppUserId"      = $appUserId
        "Version"        = "$env:RELEASE_VERSION"
        "SourceDir"      = "$env:ZED_WORKSPACE"
        "AppxFullName"   = $appAppxFullName
    }

    $defs = @()
    foreach ($key in $definitions.Keys) {
        $defs += "/d$key=`"$($definitions[$key])`""
    }

    $innoArgs = @($issFilePath) + $defs
    if ($canCodeSign) {
        $env:ZED_SIGN_BUNDLE = "1"
        $signTool = "powershell.exe -ExecutionPolicy Bypass -File $innoDir\sign.ps1 `$f"
        $innoArgs += "/sDefaultsign=`"$signTool`""
    }

    Write-Host "🚀 Running Inno Setup compiler: $innoSetupPath" -ForegroundColor Cyan
    $process = Start-Process -FilePath $innoSetupPath -ArgumentList $innoArgs -NoNewWindow -Wait -PassThru

    $expectedSetupPath = "$env:ZED_WORKSPACE\target\$appSetupName.exe"

    if ($process.ExitCode -eq 0 -and (Test-Path $expectedSetupPath)) {
        Write-Host "✅ Inno Setup successfully compiled the installer!" -ForegroundColor Green
        if ($env:GITHUB_ENV -and (Test-Path $env:GITHUB_ENV)) {
            Write-Output "SETUP_PATH=target/$appSetupName.exe" >> $env:GITHUB_ENV
        }
        $script:buildSuccess = $true
        $script:finalInstallerPath = $expectedSetupPath
    } else {
        Write-Host "❌ Inno Setup failed with exit code: $($process.ExitCode)" -ForegroundColor Red
        $script:buildSuccess = $false
    }
}

$startTime = Get-Date

ParseZedWorkspace
$innoDir = "$env:ZED_WORKSPACE\inno\$Architecture"

Write-Host "========================================================" -ForegroundColor Cyan
Write-Host " 🚀 Building Zed Windows Installer" -ForegroundColor Cyan
Write-Host " Channel:      $channel" -ForegroundColor Yellow
Write-Host " Version:      $env:RELEASE_VERSION" -ForegroundColor Yellow
Write-Host " Target Arch:  $Architecture ($target)" -ForegroundColor Yellow
Write-Host " Workspace:    $env:ZED_WORKSPACE" -ForegroundColor Yellow
Write-Host "========================================================" -ForegroundColor Cyan

CheckEnvironmentVariables
PrepareForBundle
GenerateLicenses
BuildZedAndItsFriends
BuildRemoteServer
MakeAppx
SignZedAndItsFriends
ZipZedAndItsFriendsDebug
DownloadAMDGpuServices
DownloadConpty
CollectFiles
BuildInstaller

if ($env:CI) {
    UploadToSentry
}

$elapsed = (Get-Date) - $startTime

if ($buildSuccess) {
    $fileInfo = Get-Item $script:finalInstallerPath
    $sizeMB = [math]::Round($fileInfo.Length / 1MB, 2)
    Write-Host ""
    Write-Host "========================================================" -ForegroundColor Green
    Write-Host " 🎉 Windows Installer built successfully!" -ForegroundColor Green
    Write-Host " 📦 File:     $($fileInfo.FullName)" -ForegroundColor White
    Write-Host " 📏 Size:     $sizeMB MB" -ForegroundColor White
    Write-Host " ⏱️  Duration: $($elapsed.Minutes)m $($elapsed.Seconds)s" -ForegroundColor White
    Write-Host "========================================================" -ForegroundColor Green
    Write-Host ""

    if ($Install) {
        Write-Host "🚀 Launching installer: $($fileInfo.FullName)..." -ForegroundColor Cyan
        Start-Process -FilePath $fileInfo.FullName
    }

    if ($OpenFolder) {
        explorer.exe "/select,$($fileInfo.FullName)"
    }
    exit 0
} else {
    Write-Host ""
    Write-Host "❌ Build failed after $($elapsed.Minutes)m $($elapsed.Seconds)s." -ForegroundColor Red
    exit 1
}

