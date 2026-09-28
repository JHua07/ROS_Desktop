$ErrorActionPreference = "Stop"

$rootPath = Split-Path -Parent $PSScriptRoot
$nodeBaseline = (Get-Content -LiteralPath (Join-Path $rootPath ".node-version") -Raw).Trim()
$packageJson = Get-Content -LiteralPath (Join-Path $rootPath "package.json") -Raw | ConvertFrom-Json
$rustToolchain = Get-Content -LiteralPath (Join-Path $rootPath "rust-toolchain.toml") -Raw
$pixiManifest = Get-Content -LiteralPath (Join-Path $rootPath "pixi.toml") -Raw
$rustPin = [regex]::Match($rustToolchain, 'channel\s*=\s*"([^"]+)"').Groups[1].Value

Write-Output "ROS Desktop tool baseline"
Write-Output ("  Node baseline: " + $nodeBaseline)
Write-Output ("  pnpm pin:      " + $packageJson.packageManager)
Write-Output ("  Rust pin:      " + $rustPin)
Write-Output "  Pixi channels: robostack-jazzy + conda-forge"
Write-Output "  Pixi targets:  win-64 + linux-64"

foreach ($toolName in @("node", "pnpm", "rustc", "cargo", "pixi")) {
    $command = Get-Command $toolName -ErrorAction SilentlyContinue
    if ($null -eq $command) {
        Write-Output ("  " + $toolName + ": not installed in this shell")
    } else {
        try {
            $version = & $command.Source --version 2>&1 | Select-Object -First 1
            Write-Output ("  " + $toolName + ": " + $version)
        } catch {
            Write-Output ("  " + $toolName + ": present, version check failed")
        }
    }
}
