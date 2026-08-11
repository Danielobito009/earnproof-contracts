param(
  [Parameter(Mandatory = $true)]
  [string]$Manifest,

  [switch]$AllowPlaceholders
)

$ErrorActionPreference = "Stop"

function Assert-ContractId($Name, $Value) {
  if ([string]::IsNullOrWhiteSpace($Value)) {
    throw "$Name contract ID is missing."
  }

  if ($Value -match "X{8,}" -or $Value -match "^CDX") {
    if ($AllowPlaceholders) {
      return
    }
    throw "$Name contract ID is still a placeholder."
  }

  if ($Value -notmatch "^C[A-Z2-7]{55}$") {
    throw "$Name contract ID does not look like a Stellar contract address: $Value"
  }
}

function Assert-Sha256($Name, $Value) {
  if ([string]::IsNullOrWhiteSpace($Value)) {
    throw "$Name WASM hash is missing."
  }

  if ($Value -match "0{16,}" -and -not $AllowPlaceholders) {
    throw "$Name WASM hash is still a placeholder."
  }

  if ($Value -notmatch "^[a-fA-F0-9]{64}$") {
    throw "$Name WASM hash must be a 64-character SHA-256 hex string."
  }
}

function Assert-StellarAddress($Name, $Value) {
  if ([string]::IsNullOrWhiteSpace($Value)) {
    throw "$Name address is missing."
  }

  if ($Value -notmatch "^G[A-Z2-7]{55}$") {
    throw "$Name address does not look like a Stellar public key: $Value"
  }
}

$path = Resolve-Path $Manifest
$manifestJson = Get-Content $path -Raw | ConvertFrom-Json

if ($manifestJson.network -notin @("stellar-testnet", "testnet")) {
  throw "Manifest network must be stellar-testnet or testnet."
}

if (-not $manifestJson.deployedAt) {
  throw "Manifest deployedAt timestamp is missing."
}

Assert-ContractId "protocolConfig" $manifestJson.contracts.protocolConfig
Assert-ContractId "issuerRegistry" $manifestJson.contracts.issuerRegistry
Assert-ContractId "proofRegistry" $manifestJson.contracts.proofRegistry

if ($manifestJson.initialIssuer) {
  Assert-StellarAddress "initialIssuer" $manifestJson.initialIssuer.address
  Assert-Sha256 "initialIssuer issuerIdHash" $manifestJson.initialIssuer.issuerIdHash
  Assert-Sha256 "initialIssuer metadataHash" $manifestJson.initialIssuer.metadataHash
}

if ($manifestJson.wasm) {
  Assert-Sha256 "protocolConfig" $manifestJson.wasm.protocolConfig.sha256
  Assert-Sha256 "issuerRegistry" $manifestJson.wasm.issuerRegistry.sha256
  Assert-Sha256 "proofRegistry" $manifestJson.wasm.proofRegistry.sha256
}

if (-not $manifestJson.schemaVersions -or $manifestJson.schemaVersions.Count -eq 0) {
  throw "At least one schema version must be listed."
}

Write-Host "Deployment manifest is valid: $path"
