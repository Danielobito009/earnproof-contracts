################################################################################
# upgrade-contract.ps1
#
# Governed in-place WASM upgrade for a single EarnProof Soroban contract.
#
# Workflow enforced by this script:
#   1. Build fresh WASM artifacts.
#   2. Upload the target WASM to the Stellar network and capture its hash.
#   3. Call approve_upgrade(wasm_hash, new_version) on the target contract.
#   4. Optionally pause protocol-config to gate new proof registrations during
#      the upgrade window (--PauseProtocol).
#   5. Call upgrade_contract(wasm_hash) to apply the upgrade.
#   6. Optionally unpause protocol-config (--PauseProtocol).
#   7. Verify the contract's contract_version matches new_version.
#   8. Write an upgrade manifest recording hash, version, and provenance.
#
# Dry-run mode (--DryRun) executes steps 1-2 (build + upload) and emits what
# would be executed without making any on-chain state changes.  Use this for
# rehearsal against a local/test network before a live upgrade.
#
# Usage:
#   .\scripts\upgrade-contract.ps1 `
#     -Contract protocol-config `
#     -ContractId CC3OREX5... `
#     -Source earnproof-deployer `
#     -NewVersion 2 `
#     -Network testnet
#
#   # Dry run only (no on-chain changes):
#   .\scripts\upgrade-contract.ps1 ... -DryRun
#
#   # Pause protocol during upgrade window:
#   .\scripts\upgrade-contract.ps1 ... -PauseProtocol `
#     -ProtocolConfigId CC3OREX5...
################################################################################

param(
  [Parameter(Mandatory = $true)]
  [ValidateSet("protocol-config", "issuer-registry", "proof-registry")]
  [string]$Contract,

  [Parameter(Mandatory = $true)]
  [string]$ContractId,

  [Parameter(Mandatory = $true)]
  [string]$Source,

  [Parameter(Mandatory = $true)]
  [int]$NewVersion,

  [string]$Network = "testnet",

  # When set, pause protocol-config before upgrading and unpause after.
  [switch]$PauseProtocol,
  [string]$ProtocolConfigId = "",

  # Dry-run: build and upload WASM, print what would be invoked, then exit.
  [switch]$DryRun,

  [string]$Output = "",
  [int]$MaxRetries = 5
)

$ErrorActionPreference = "Stop"

function Assert-Command($Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Required command '$Name' was not found."
  }
}

function Test-RetryableStellarError($Output) {
  $text = ($Output -join "`n")
  return $text -match "SendRequest|Connect|connection|timeout|timed out|temporarily unavailable|TxBadSeq"
}

function Invoke-WithRetry($Description, $Command, [switch]$CaptureOutput) {
  $attempt = 1
  $delaySeconds = 2
  while ($true) {
    Write-Host "==> $Description"
    if ($attempt -gt 1) { Write-Host "    retry $attempt of $MaxRetries" }
    $output = & $Command[0] @($Command | Select-Object -Skip 1) 2>&1
    $exitCode = $LASTEXITCODE
    if ($exitCode -eq 0) {
      if ($CaptureOutput) { return $output }
      if ($output) { $output | ForEach-Object { Write-Host $_ } }
      return
    }
    if ($output) { $output | ForEach-Object { Write-Warning $_ } }
    if (($attempt -ge $MaxRetries) -or -not (Test-RetryableStellarError $output)) {
      throw "Command failed after $attempt attempt(s): $($Command -join ' ')"
    }
    Write-Warning "Retryable Stellar RPC error. Waiting $delaySeconds second(s)."
    Start-Sleep -Seconds $delaySeconds
    $attempt += 1
    $delaySeconds = [Math]::Min($delaySeconds * 2, 30)
  }
}

function Invoke-Step($Description, $Command) {
  Invoke-WithRetry $Description $Command
}

function Invoke-Capture($Description, $Command) {
  $result = Invoke-WithRetry $Description $Command -CaptureOutput
  foreach ($line in $result) {
    $trimmed = "$line".Trim()
    # Soroban WASM hash is a 64-char lowercase hex string
    if ($trimmed -match "^([0-9a-f]{64})$") { return $Matches[1] }
  }
  throw "Could not find WASM hash in output: $($result -join ' ')"
}

function Get-Sha256($Path) {
  return (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Read-ContractVersion($ContractId, $Network, $Source) {
  $raw = Invoke-WithRetry "Read contract_version from $ContractId" @(
    "stellar", "contract", "invoke",
    "--source", $Source, "--network", $Network,
    "--id", $ContractId, "--", "get_contract_version"
  ) -CaptureOutput
  $val = ($raw -join "") -replace '[^0-9]', ''
  return [int]$val
}

Assert-Command "cargo"
Assert-Command "rustup"
Assert-Command "stellar"

if ($PauseProtocol -and -not $ProtocolConfigId) {
  throw "-PauseProtocol requires -ProtocolConfigId to be set."
}

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $root

try {
  # ── Map contract name to WASM artifact ──────────────────────────────────────
  $wasmName = switch ($Contract) {
    "protocol-config"  { "protocol_config" }
    "issuer-registry"  { "issuer_registry" }
    "proof-registry"   { "proof_registry" }
  }
  $wasmPath = Join-Path $root "target/wasm32v1-none/release/$wasmName.wasm"

  # ── Step 1: Build ────────────────────────────────────────────────────────────
  Invoke-Step "Install wasm32v1-none target" @("rustup", "target", "add", "wasm32v1-none")
  Invoke-Step "Build WASM artifacts" @("stellar", "contract", "build")

  if (-not (Test-Path $wasmPath)) {
    throw "Expected WASM artifact not found: $wasmPath"
  }

  $wasmSha256 = Get-Sha256 $wasmPath
  Write-Host "WASM sha256: $wasmSha256"

  # ── Step 2: Upload WASM to network ───────────────────────────────────────────
  $wasmHash = Invoke-Capture "Upload $Contract WASM" @(
    "stellar", "contract", "upload",
    "--source", $Source, "--network", $Network,
    "--wasm", $wasmPath
  )
  Write-Host "Uploaded WASM hash: $wasmHash"

  # ── Dry-run exit ─────────────────────────────────────────────────────────────
  if ($DryRun) {
    Write-Host ""
    Write-Host "=== DRY RUN — no on-chain state changes made ==="
    Write-Host ""
    Write-Host "Would invoke on contract $ContractId (network: $Network):"
    Write-Host "  approve_upgrade --wasm_hash $wasmHash --new_version $NewVersion"
    if ($PauseProtocol) {
      Write-Host "  (protocol-config $ProtocolConfigId) pause"
    }
    Write-Host "  upgrade_contract --wasm_hash $wasmHash"
    if ($PauseProtocol) {
      Write-Host "  (protocol-config $ProtocolConfigId) unpause"
    }
    Write-Host "  get_contract_version  -- expected: $NewVersion"
    Write-Host ""
    Write-Host "Rehearsal complete.  Re-run without -DryRun to apply."
    return
  }

  # ── Step 3: Allowlist the WASM hash ─────────────────────────────────────────
  Invoke-Step "Allowlist WASM hash (approve_upgrade)" @(
    "stellar", "contract", "invoke",
    "--source", $Source, "--network", $Network,
    "--auth-mode", "root", "--auto-sign",
    "--id", $ContractId, "--",
    "approve_upgrade",
    "--wasm_hash", $wasmHash,
    "--new_version", "$NewVersion"
  )

  # ── Step 4 (optional): Pause protocol ───────────────────────────────────────
  if ($PauseProtocol) {
    Invoke-Step "Pause protocol-config" @(
      "stellar", "contract", "invoke",
      "--source", $Source, "--network", $Network,
      "--auth-mode", "root", "--auto-sign",
      "--id", $ProtocolConfigId, "--", "pause"
    )
  }

  # ── Step 5: Apply upgrade ────────────────────────────────────────────────────
  Invoke-Step "Apply upgrade (upgrade_contract)" @(
    "stellar", "contract", "invoke",
    "--source", $Source, "--network", $Network,
    "--auth-mode", "root", "--auto-sign",
    "--id", $ContractId, "--",
    "upgrade_contract",
    "--wasm_hash", $wasmHash
  )

  # ── Step 6 (optional): Unpause protocol ─────────────────────────────────────
  if ($PauseProtocol) {
    Invoke-Step "Unpause protocol-config" @(
      "stellar", "contract", "invoke",
      "--source", $Source, "--network", $Network,
      "--auth-mode", "root", "--auto-sign",
      "--id", $ProtocolConfigId, "--", "unpause"
    )
  }

  # ── Step 7: Verify version ───────────────────────────────────────────────────
  $onChainVersion = Read-ContractVersion $ContractId $Network $Source
  if ($onChainVersion -ne $NewVersion) {
    throw "Version mismatch after upgrade: on-chain=$onChainVersion expected=$NewVersion"
  }
  Write-Host "contract_version verified: $onChainVersion"

  # ── Step 8: Write upgrade manifest ──────────────────────────────────────────
  $gitCommit = (& git rev-parse HEAD 2>&1) 2>$null
  if (-not $gitCommit -or $LASTEXITCODE -ne 0) { $gitCommit = "unknown" }

  $upgradeManifest = [ordered]@{
    contract        = $Contract
    contractId      = $ContractId
    network         = "stellar-$Network"
    upgradedAt      = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    source          = $Source
    newVersion      = $NewVersion
    wasm            = [ordered]@{
      path            = "target/wasm32v1-none/release/$wasmName.wasm"
      sha256          = $wasmSha256
      uploadedHash    = $wasmHash
      contractVersion = $NewVersion
      buildMetadata   = [ordered]@{
        rustToolchain       = "stable"
        cargoPackageVersion = "0.1.0"
        sorobanSdkVersion   = "27.0.0"
        buildProfile        = "release"
        gitCommit           = "$gitCommit"
      }
    }
    pausedDuring    = $PauseProtocol.IsPresent
  }

  if (-not $Output) {
    $ts = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    $Output = "scripts/upgrade-manifest.$Contract.$ts.json"
  }
  $outputPath = Join-Path $root $Output
  $upgradeManifest | ConvertTo-Json -Depth 10 | Set-Content -Path $outputPath -Encoding UTF8
  Write-Host "Wrote upgrade manifest: $outputPath"
}
finally {
  Pop-Location
}
