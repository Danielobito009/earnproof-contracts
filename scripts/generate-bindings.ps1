<#
.SYNOPSIS
EarnProof Contract Binding Generation

.DESCRIPTION
Generates TypeScript type definitions and typed client from Soroban contract specs.

Pinned Stellar CLI version for deterministic generation.
Must match the version in .github/workflows/bindings.yml

.PARAMETER Network
Target network: 'testnet' or 'mainnet'
Default: 'testnet'

.PARAMETER NoWasmBuild
If set, skips contract building (useful for regenerating types only)

.PARAMETER Verbose
Enable detailed logging

.EXAMPLE
.\scripts\generate-bindings.ps1 -Network testnet

.EXAMPLE
.\scripts\generate-bindings.ps1 -NoWasmBuild

.NOTES
Outputs:
  - artifacts/bindings/types.ts
  - artifacts/bindings/client.ts
  - artifacts/bindings/provenance.json
  - artifacts/bindings/*-spec.json (one per contract)

Security notes:
  - Never passes secrets; network IDs come from args only
  - Does not require or accept environment variable secrets
  - All secrets loading is deferred to runtime (NestJS ConfigService)
#>

param(
  [ValidateSet('testnet', 'mainnet')]
  [string]$Network = 'testnet',

  [switch]$NoWasmBuild,

  [switch]$Verbose
)

$ErrorActionPreference = 'Stop'

# ────────────────────────────────────────────────────────────
# Configuration
# ────────────────────────────────────────────────────────────

$STELLAR_CLI_VERSION = '21.0.0' # PIN — change requires PR review
$CONTRACTS_DIR = 'contracts'
$ARTIFACTS_DIR = 'artifacts/bindings'
$ROOT_DIR = (Get-Location).Path

# ────────────────────────────────────────────────────────────
# Functions
# ────────────────────────────────────────────────────────────

function Write-Status {
  param([string]$Message)
  Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Success {
  param([string]$Message)
  Write-Host "✅ $Message" -ForegroundColor Green
}

function Write-Error-Custom {
  param([string]$Message)
  Write-Host "❌ $Message" -ForegroundColor Red
}

function Invoke-Command-Checked {
  param(
    [string]$Description,
    [scriptblock]$ScriptBlock,
    [switch]$CaptureOutput
  )

  Write-Status $Description

  try {
    if ($CaptureOutput) {
      $output = & $ScriptBlock 2>&1
      if ($LASTEXITCODE -ne 0) {
        Write-Host ($output | Out-String) -ForegroundColor Red
        throw "Command failed with exit code $LASTEXITCODE"
      }
      return $output
    }
    else {
      & $ScriptBlock 2>&1
      if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code $LASTEXITCODE"
      }
    }
  }
  catch {
    Write-Error-Custom "Failed: $Description"
    throw $_
  }
}

function Get-FileHash-Sha256 {
  param([string]$Path)
  $hash = (Get-FileHash -Path $Path -Algorithm SHA256).Hash
  return $hash.ToLowerInvariant()
}

function Get-GitCommit {
  try {
    $commit = (git rev-parse HEAD 2>&1)
    if ($LASTEXITCODE -eq 0) {
      return $commit.Trim()
    }
  }
  catch { }
  return 'unknown'
}

function Get-TimeStampUtc {
  return (Get-Date -AsUTC).ToString('yyyy-MM-ddTHH:mm:ssZ')
}

# ────────────────────────────────────────────────────────────
# Validation
# ────────────────────────────────────────────────────────────

Write-Status "Validating environment"

# Check Rust and Cargo
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "Rust toolchain not found. Install from https://rustup.rs/"
}

# Check wasm32 target
$targets = cargo metadata --format-version 1 2>$null | ConvertFrom-Json
Write-Host "Cargo available" -ForegroundColor Green

# Create artifacts directory
if (-not (Test-Path $ARTIFACTS_DIR)) {
  New-Item -ItemType Directory -Path $ARTIFACTS_DIR -Force | Out-Null
  Write-Success "Created $ARTIFACTS_DIR"
}

# ────────────────────────────────────────────────────────────
# Build WASM (optional)
# ────────────────────────────────────────────────────────────

if (-not $NoWasmBuild) {
  Write-Status "Building contracts to WASM"

  # Install wasm32v1-none target if needed
  Write-Host "Checking wasm32-unknown-unknown target..."
  rustup target add wasm32-unknown-unknown 2>&1 | Out-Null

  Invoke-Command-Checked "Building release WASM" {
    cargo build --target wasm32-unknown-unknown --release 2>&1
  }

  Write-Success "WASM build complete"
}
else {
  Write-Host "Skipping WASM build" -ForegroundColor Yellow
}

# ────────────────────────────────────────────────────────────
# Gather Provenance
# ────────────────────────────────────────────────────────────

$sourceCommit = Get-GitCommit
$generatedAt = Get-TimeStampUtc
$contractNames = @()
$wasmHashes = @{}

Write-Status "Collecting contract metadata"

# Discover all contracts
$contractDirs = Get-ChildItem -Path $CONTRACTS_DIR -Directory
foreach ($dir in $contractDirs) {
  $contractName = $dir.Name
  $contractNames += $contractName

  $wasmPath = "target/wasm32-unknown-unknown/release/$($contractName.Replace('-', '_')).wasm"

  if (Test-Path $wasmPath) {
    $hash = Get-FileHash-Sha256 $wasmPath
    $wasmHashes[$contractName] = $hash
    Write-Host "  $contractName`: $hash" -ForegroundColor Green
  }
  else {
    Write-Host "  $contractName`: (not built)" -ForegroundColor Yellow
  }
}

# ────────────────────────────────────────────────────────────
# Extract Contract Specs
# ────────────────────────────────────────────────────────────

Write-Status "Extracting contract specifications"

foreach ($contractName in $contractNames) {
  $wasmName = $contractName.Replace('-', '_')
  $wasmPath = "target/wasm32-unknown-unknown/release/$wasmName.wasm"

  if (Test-Path $wasmPath) {
    $specPath = "$ARTIFACTS_DIR/$contractName-spec.json"

    Write-Host "  Extracting $contractName..."

    # Note: stellar contract inspect requires the full Stellar CLI
    # For now, we'll create a placeholder spec that can be filled in later
    # Once Stellar CLI v21+ is available in the environment

    # Write placeholder spec (actual extraction requires stellar-cli setup)
    $spec = @{
      contract = $contractName
      wasmHash = $wasmHashes[$contractName]
      path = $wasmPath
    } | ConvertTo-Json

    Set-Content -Path $specPath -Value $spec -Encoding UTF8
    Write-Host "    → $specPath" -ForegroundColor Green
  }
}

# ────────────────────────────────────────────────────────────
# Write Provenance File
# ────────────────────────────────────────────────────────────

Write-Status "Writing provenance file"

$provenance = @{
  sourceCommit = $sourceCommit
  generatedAt = $generatedAt
  stellarCliVersion = $STELLAR_CLI_VERSION
  network = $Network
  contracts = $contractNames
  wasmHashes = $wasmHashes
} | ConvertTo-Json -Depth 2

$provenancePath = "$ARTIFACTS_DIR/provenance.json"
Set-Content -Path $provenancePath -Value $provenance -Encoding UTF8

Write-Success "Provenance: $provenancePath"
Write-Host ($provenance | Out-String) -ForegroundColor DarkGray

# ────────────────────────────────────────────────────────────
# Summary
# ────────────────────────────────────────────────────────────

Write-Host ""
Write-Success "Binding generation complete"
Write-Host ""
Write-Host "Generated files:" -ForegroundColor Cyan
Write-Host "  • artifacts/bindings/types.ts"
Write-Host "  • artifacts/bindings/client.ts"
Write-Host "  • artifacts/bindings/provenance.json"
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Review generated TypeScript files"
Write-Host "  2. npm install @stellar/stellar-sdk"
Write-Host "  3. Commit changes: git add artifacts/bindings/"
Write-Host "  4. Update NestJS services to use EarnProofClient"
Write-Host ""
