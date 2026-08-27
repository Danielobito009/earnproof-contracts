#!/usr/bin/env pwsh
#
# Measure contract WASM sizes and report potential regressions.
#
# This script builds all contracts in release mode and measures their
# optimized WASM sizes. Size increases may indicate unnecessary code
# bloat or unintended dependencies.
#
# Thresholds include 10% headroom above baseline measurements.
#
# Usage:
#   ./scripts/measure-resources.ps1
#
# To update baselines after intentional changes:
# 1. Review the size changes and ensure they are justified
# 2. Update the MAX_SIZE constants below
# 3. Document the change in the PR description

param(
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"

Write-Host "=== EarnProof Contract Resource Measurement ===" -ForegroundColor Cyan
Write-Host ""

# WASM size thresholds (bytes)
# These values include ~10% headroom above current optimized sizes
$MAX_PROTOCOL_CONFIG_SIZE = 5000
$MAX_ISSUER_REGISTRY_SIZE = 8000
$MAX_PROOF_REGISTRY_SIZE = 9000

# Contract paths
$contracts = @{
    "protocol-config" = @{
        path = "contracts/protocol-config"
        max_size = $MAX_PROTOCOL_CONFIG_SIZE
    }
    "issuer-registry" = @{
        path = "contracts/issuer-registry"
        max_size = $MAX_ISSUER_REGISTRY_SIZE
    }
    "proof-registry" = @{
        path = "contracts/proof-registry"
        max_size = $MAX_PROOF_REGISTRY_SIZE
    }
}

# Build all contracts in release mode
Write-Host "Building contracts in release mode..." -ForegroundColor Yellow
cargo build --release --workspace --target wasm32-unknown-unknown
if ($LASTEXITCODE -ne 0) {
    Write-Error "Build failed"
    exit 1
}

Write-Host "Build successful" -ForegroundColor Green
Write-Host ""

# Measure and report sizes
$failed = $false
$results = @()

foreach ($name in $contracts.Keys) {
    $config = $contracts[$name]
    $wasm_path = "target/wasm32-unknown-unknown/release/$name.wasm"
    
    if (-not (Test-Path $wasm_path)) {
        Write-Host "  ERROR: WASM file not found: $wasm_path" -ForegroundColor Red
        $failed = $true
        continue
    }
    
    $size = (Get-Item $wasm_path).Length
    $max_size = $config.max_size
    $percent_of_max = [math]::Round(($size / $max_size) * 100, 1)
    
    $result = [PSCustomObject]@{
        Contract = $name
        Size = $size
        MaxSize = $max_size
        PercentOfMax = $percent_of_max
        Status = if ($size -le $max_size) { "PASS" } else { "FAIL" }
    }
    
    $results += $result
    
    $status_color = if ($result.Status -eq "PASS") { "Green" } else { "Red" }
    $status_symbol = if ($result.Status -eq "PASS") { "[PASS]" } else { "[FAIL]" }
    
    Write-Host "  $status_symbol $name" -ForegroundColor $status_color
    Write-Host "      Size: $size bytes" -ForegroundColor Gray
    Write-Host "      Max:  $max_size bytes" -ForegroundColor Gray
    Write-Host "      Usage: $percent_of_max%" -ForegroundColor Gray
    
    if ($result.Status -eq "FAIL") {
        $excess = $size - $max_size
        $excess_percent = [math]::Round((($size / $max_size) - 1) * 100, 1)
        Write-Host "      REGRESSION: +$excess bytes (+$excess_percent percent)" -ForegroundColor Red
        $failed = $true
    }
    
    Write-Host ""
}

# Print summary table
Write-Host ""
Write-Host "=== Summary ===" -ForegroundColor Cyan
Write-Host ""
$results | Format-Table -Property Contract, Size, MaxSize, PercentOfMax, Status -AutoSize

# Print detailed info if verbose
if ($Verbose) {
    Write-Host ""
    Write-Host "=== Detailed Information ===" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Toolchain:"
    rustc --version
    Write-Host ""
    Write-Host "Soroban SDK version:"
    cargo tree --workspace -p soroban-sdk | Select-Object -First 1
    Write-Host ""
}

# Exit with failure if any contract exceeded threshold
if ($failed) {
    Write-Host "Resource regression detected!" -ForegroundColor Red
    Write-Host ""
    Write-Host "If this increase is intentional:" -ForegroundColor Yellow
    Write-Host "  1. Review and document the reason for the size increase" -ForegroundColor Yellow
    Write-Host "  2. Update thresholds in scripts/measure-resources.ps1" -ForegroundColor Yellow
    Write-Host "  3. Include justification in your PR description" -ForegroundColor Yellow
    exit 1
}

Write-Host "All contracts within resource budgets [PASS]" -ForegroundColor Green
exit 0
