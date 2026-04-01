# Parameters
param (
    [switch]$Local,
    [string]$Path = "",
    [string]$Repo = "nimblemo/code-tree-rs"
)

# Configuration
$BinaryName = "code-tree-rs"
$InstallDir = if ($Local) { $PWD.Path } elseif ($Path) { $Path } else { Join-Path $HOME ".code-tree-rs\bin" }
$ZipFile = Join-Path $env:TEMP "$BinaryName.zip"

# Create target directory if it doesn't exist
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Write-Host "Creating installation directory: $InstallDir" -ForegroundColor Cyan
}

# Fetch latest release info
Write-Host "Fetching latest release from $Repo..." -ForegroundColor Cyan
try {
    $ReleaseInfo = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Asset = $ReleaseInfo.assets | Where-Object { $_.name -like "*windows-msvc.zip" } | Select-Object -First 1

    if (-not $Asset) {
        Write-Error "No Windows build found in the latest release. Please check $Repo releases."
        exit 1
    }

    $DownloadUrl = $Asset.browser_download_url
    Write-Host "Found version: $($ReleaseInfo.tag_name)" -ForegroundColor Green
} catch {
    Write-Error "Failed to fetch release info: $_"
    exit 1
}

# Download and extract
Write-Host "Downloading $BinaryName from $DownloadUrl..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipFile

Write-Host "Extracting to $InstallDir..." -ForegroundColor Cyan
Expand-Archive -Path $ZipFile -DestinationPath $InstallDir -Force
Remove-Item $ZipFile

# Check for existence of the binary after extraction
$BinaryPath = Join-Path $InstallDir "$BinaryName.exe"
if (-not (Test-Path $BinaryPath)) {
    # Sometimes zip contains a subfolder or different name, let's try to find it
    $Files = Get-ChildItem -Path $InstallDir -Filter "*.exe" -Recurse
    if ($Files.Count -gt 0) {
        $BinaryPath = $Files[0].FullName
        Write-Host "Detected binary at: $BinaryPath" -ForegroundColor Yellow
    } else {
        Write-Error "Could not find binary executable in the extracted archive."
        exit 1
    }
}

# Add to PATH if not local
if (-not $Local) {
    $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($UserPath -notlike "*$InstallDir*") {
        Write-Host "Adding $InstallDir to user PATH..." -ForegroundColor Cyan
        $NewPath = "$UserPath;$InstallDir"
        [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
        $env:PATH += ";$InstallDir"
        Write-Host "Successfully added to PATH. Please restart your terminal!" -ForegroundColor Green
    } else {
        Write-Host "$InstallDir is already in your PATH." -ForegroundColor Yellow
    }
} else {
    Write-Host "Local installation complete. Binary is available at: $BinaryPath" -ForegroundColor Green
}

Write-Host "`nInstallation of $BinaryName finished successfully! ✨" -ForegroundColor Green
