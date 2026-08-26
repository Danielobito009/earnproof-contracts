#Requires -Version 7.0
<#
.SYNOPSIS
  Deploys all EarnProof contracts to a local Soroban sandbox and exercises a
  synthetic proof lifecycle.

.DESCRIPTION
  Fills the gap between `cargo test` and a testnet deployment. Unit tests run
  contracts in-process and never exercise WASM installation, contract IDs, or
  cross-contract wiring; a testnet deployment exercises all of that but costs
  real time, needs funded accounts, and leaves state behind.

  One command:
    1. validates prerequisites;
    2. builds optimized WASM;
    3. deploys protocol-config, issuer-registry, proof-registry in dependency
       order;
    4. initializes admins and approves a schema version;
    5. exercises issuer registration, proof registration, verification,
       revocation, and pause behaviour;
    6. writes a disposable manifest.

  EVERY value this script uses is synthetic and generated locally. It never
  reads testnet or mainnet credentials, and it refuses to run against any
  network other than `local`. See the "Safety" section below.

.PARAMETER Network
  Stellar CLI network name. Defaults to `local`. Any other value is rejected —
  this harness must never touch a shared network.

.PARAMETER Output
  Where to write the disposable manifest. Defaults to a gitignored path.

.PARAMETER KeepState
  Keep the sandbox identity and container running after the run. Without it,
  the generated identity is removed so repeat runs start clean.

.PARAMETER SkipBuild
  Reuse existing WASM artifacts instead of rebuilding. Useful when iterating on
  the harness itself rather than on the contracts.

.EXAMPLE
  pwsh -File scripts/local-sandbox/run-sandbox.ps1

.EXAMPLE
  pwsh -File scripts/local-sandbox/run-sandbox.ps1 -KeepState -SkipBuild
#>

param(
  [string]$Network = "local",
  [string]$Output = "scripts/local-sandbox/.sandbox-manifest.json",
  [switch]$KeepState,
  [switch]$SkipBuild,
  [int]$MaxRetries = 5
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# Safety
# ---------------------------------------------------------------------------
# The harness generates throwaway keys and funds them from the local friendbot.
# Pointing it at a shared network would mean generating an identity there and
# deploying unreviewed artifacts under it. The guard is first so no other work
# happens before it.

if ($Network -ne "local") {
  throw @"
This harness only runs against the local sandbox network.

Requested network: '$Network'

It generates throwaway identities and deploys unreviewed artifacts, neither of
which belongs on a shared network. For testnet, use scripts/deploy-testnet.ps1,
which takes an existing funded identity you control.
"@
}

# Identity name is fixed and obviously disposable, so it cannot be confused
# with a real deployer profile in `stellar keys ls`.
$IdentityName = "earnproof-sandbox-throwaway"

# ---------------------------------------------------------------------------
# Prerequisites
# ---------------------------------------------------------------------------

function Assert-Command($Name, $Hint) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Required command '$Name' was not found. $Hint"
  }
}

function Test-RetryableStellarError($Output) {
  $text = "$Output"
  $patterns = @(
    "connection reset",
    "connection refused",
    "temporarily unavailable",
    "send failure",
    "timeout",
    "timed out",
    "sequence",
    "503",
    "502",
    "504"
  )

  foreach ($pattern in $patterns) {
    if ($text -imatch $pattern) { return $true }
  }
  return $false
}

# Mirrors the retry behaviour of scripts/deploy-testnet.ps1 so both scripts
# behave the same way when the RPC is briefly unavailable — a local container
# that has just started is exactly that case.
function Invoke-WithRetry($Description, $Command, [switch]$CaptureOutput) {
  $attempt = 1
  $delaySeconds = 2

  while ($true) {
    Write-Host "==> $Description"
    if ($attempt -gt 1) {
      Write-Host "    retry $attempt of $MaxRetries"
    }

    $output = & $Command[0] @($Command | Select-Object -Skip 1) 2>&1
    $exitCode = $LASTEXITCODE

    if ($exitCode -eq 0) {
      if ($CaptureOutput) { return $output }
      if ($output) { $output | ForEach-Object { Write-Host "    $_" } }
      return
    }

    if ($output) { $output | ForEach-Object { Write-Warning $_ } }

    if (($attempt -ge $MaxRetries) -or -not (Test-RetryableStellarError $output)) {
      throw "Command failed after $attempt attempt(s): $($Command -join ' ')"
    }

    Write-Warning "Retryable error. Waiting $delaySeconds second(s) before retrying."
    Start-Sleep -Seconds $delaySeconds
    $attempt += 1
    $delaySeconds = [Math]::Min($delaySeconds * 2, 30)
  }
}

function Invoke-Capture($Description, $Command) {
  $result = Invoke-WithRetry $Description $Command -CaptureOutput

  foreach ($line in $result) {
    $trimmed = "$line".Trim()
    if ($trimmed -match "^(C[A-Z2-7]{55})$") { return $Matches[1] }
    if ($trimmed -match "/contract/(C[A-Z2-7]{55})") { return $Matches[1] }
  }

  throw "Could not find a contract ID in command output: $($result -join ' ')"
}

# Reads a value back from a deployed contract.
function Invoke-Read($Description, $ContractId, $Function, $Arguments = @()) {
  $command = @(
    "stellar", "contract", "invoke",
    "--source", $IdentityName,
    "--network", $Network,
    "--id", $ContractId,
    "--"
    $Function
  ) + $Arguments

  $result = Invoke-WithRetry $Description $command -CaptureOutput
  return ("$result").Trim()
}

function Invoke-Write($Description, $ContractId, $Function, $Arguments = @()) {
  $command = @(
    "stellar", "contract", "invoke",
    "--source", $IdentityName,
    "--network", $Network,
    "--auth-mode", "root",
    "--auto-sign",
    "--id", $ContractId,
    "--"
    $Function
  ) + $Arguments

  Invoke-WithRetry $Description $command | Out-Null
}

# Invokes something expected to fail, and fails the run if it succeeds.
# This is what makes the pause and revocation checks meaningful rather than
# decorative.
function Assert-Rejected($Description, $ContractId, $Function, $Arguments = @()) {
  Write-Host "==> $Description (expecting rejection)"

  $command = @(
    "stellar", "contract", "invoke",
    "--source", $IdentityName,
    "--network", $Network,
    "--auth-mode", "root",
    "--auto-sign",
    "--id", $ContractId,
    "--"
    $Function
  ) + $Arguments

  $output = & $command[0] @($command | Select-Object -Skip 1) 2>&1
  $exitCode = $LASTEXITCODE

  if ($exitCode -eq 0) {
    throw "$Description succeeded but should have been rejected."
  }

  Write-Host "    correctly rejected"
}

function Get-Sha256Text($Value) {
  $bytes = [System.Text.Encoding]::UTF8.GetBytes($Value)
  $sha256 = [System.Security.Cryptography.SHA256]::Create()
  try {
    $hash = $sha256.ComputeHash($bytes)
    return [System.BitConverter]::ToString($hash).Replace("-", "").ToLowerInvariant()
  }
  finally { $sha256.Dispose() }
}

function Get-Sha256File($Path) {
  return (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

Assert-Command "cargo"  "Install Rust from https://rustup.rs."
Assert-Command "rustup" "Install Rust from https://rustup.rs."
Assert-Command "stellar" "Install the Stellar CLI: cargo install --locked stellar-cli."

$root = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
Push-Location $root

try {
  # --- Network -------------------------------------------------------------
  # `stellar container start local` runs a local quickstart node. If the
  # network is already reachable the container is left alone, which is what
  # makes repeat runs cheap.
  Write-Host "==> Checking the local sandbox network"

  $networkReady = $false
  try {
    $null = & stellar network ls 2>&1
    if ($LASTEXITCODE -eq 0) { $networkReady = $true }
  }
  catch { $networkReady = $false }

  if (-not $networkReady) {
    throw @"
The Stellar CLI could not list networks.

Start a local sandbox first:
  stellar container start local

That requires Docker or Podman. See docs/local-development.md.
"@
  }

  # --- Identity ------------------------------------------------------------
  # Generated fresh and funded from the local friendbot. No key material is
  # read from the environment, and nothing is written outside .stellar/, which
  # is gitignored.
  Write-Host "==> Preparing the throwaway sandbox identity"

  $existing = & stellar keys ls 2>&1
  if ("$existing" -notmatch [regex]::Escape($IdentityName)) {
    Invoke-WithRetry "Generate sandbox identity" @(
      "stellar", "keys", "generate", "--network", $Network, "--fund", $IdentityName
    )
  }
  else {
    Write-Host "    reusing existing sandbox identity"
  }

  $adminAddress = (Invoke-WithRetry "Read sandbox identity address" @(
      "stellar", "keys", "address", $IdentityName
    ) -CaptureOutput | Out-String).Trim()

  if ($adminAddress -notmatch "^G[A-Z2-7]{55}$") {
    throw "Could not read a valid address for the sandbox identity: $adminAddress"
  }

  # --- Build ---------------------------------------------------------------
  if (-not $SkipBuild) {
    Invoke-WithRetry "Install the Stellar WASM target" @(
      "rustup", "target", "add", "wasm32v1-none"
    )
    Invoke-WithRetry "Build contract WASM artifacts" @("stellar", "contract", "build")
  }
  else {
    Write-Host "==> Skipping build (-SkipBuild)"
  }

  $wasmRoot = Join-Path $root "target/wasm32v1-none/release"
  $protocolWasm = Join-Path $wasmRoot "protocol_config.wasm"
  $issuerWasm = Join-Path $wasmRoot "issuer_registry.wasm"
  $proofWasm = Join-Path $wasmRoot "proof_registry.wasm"

  foreach ($wasm in @($protocolWasm, $issuerWasm, $proofWasm)) {
    if (-not (Test-Path $wasm)) {
      throw "Expected WASM artifact was not found: $wasm`nRun without -SkipBuild."
    }
  }

  # --- Deploy --------------------------------------------------------------
  # Order matters: proof-registry takes the other two addresses at
  # initialization and has no setter, so they must exist first.
  $protocolId = Invoke-Capture "Deploy protocol-config" @(
    "stellar", "contract", "deploy",
    "--source", $IdentityName, "--network", $Network, "--wasm", $protocolWasm
  )
  $issuerId = Invoke-Capture "Deploy issuer-registry" @(
    "stellar", "contract", "deploy",
    "--source", $IdentityName, "--network", $Network, "--wasm", $issuerWasm
  )
  $proofId = Invoke-Capture "Deploy proof-registry" @(
    "stellar", "contract", "deploy",
    "--source", $IdentityName, "--network", $Network, "--wasm", $proofWasm
  )

  # --- Initialize ----------------------------------------------------------
  Invoke-Write "Initialize protocol-config" $protocolId "initialize" @("--admin", $adminAddress)
  Invoke-Write "Approve schema version 1" $protocolId "approve_schema_version" @("--version", "1")
  Invoke-Write "Initialize issuer-registry" $issuerId "initialize" @("--admin", $adminAddress)
  Invoke-Write "Initialize proof-registry" $proofId "initialize" @(
    "--admin", $adminAddress,
    "--issuer_registry", $issuerId,
    "--protocol_config", $protocolId
  )

  # --- Synthetic lifecycle -------------------------------------------------
  # Every hash below is derived from a fixed literal string. Nothing here is a
  # real wallet, proof, or credential, and the derivation is printed so a
  # reader can confirm that for themselves.
  $issuerIdHash = Get-Sha256Text "earnproof-sandbox:issuer:1"
  $issuerMetadataHash = Get-Sha256Text "earnproof-sandbox:metadata:1"
  $proofIdHash = Get-Sha256Text "earnproof-sandbox:proof:1"
  $commitmentHash = Get-Sha256Text "earnproof-sandbox:commitment:1"
  $secondProofIdHash = Get-Sha256Text "earnproof-sandbox:proof:2"
  $secondCommitment = Get-Sha256Text "earnproof-sandbox:commitment:2"

  # Far enough ahead that the run cannot race the expiry check.
  $expiresAt = [DateTimeOffset]::UtcNow.AddDays(30).ToUnixTimeSeconds()

  Write-Host ""
  Write-Host "=== Lifecycle ===" -ForegroundColor Cyan

  Invoke-Write "Register the synthetic issuer" $issuerId "register_issuer" @(
    "--issuer_id_hash", $issuerIdHash,
    "--issuer_address", $adminAddress,
    "--metadata_hash", $issuerMetadataHash
  )

  $issuerActive = Invoke-Read "Verify the issuer is active" $issuerId "is_active_issuer" @(
    "--issuer_id_hash", $issuerIdHash
  )
  if ($issuerActive -notmatch "true") {
    throw "Issuer should be active after registration, got: $issuerActive"
  }

  Invoke-Write "Register a synthetic proof" $proofId "register_proof" @(
    "--proof_id_hash", $proofIdHash,
    "--commitment_hash", $commitmentHash,
    "--issuer_address", $adminAddress,
    "--schema_version", "1",
    "--expires_at", "$expiresAt"
  )

  $valid = Invoke-Read "Verify the proof is valid" $proofId "is_valid_proof" @(
    "--proof_id_hash", $proofIdHash
  )
  if ($valid -notmatch "true") {
    throw "Proof should be valid after registration, got: $valid"
  }

  # --- Pause behaviour -----------------------------------------------------
  # The property worth exercising here is the asymmetry: pausing blocks new
  # registrations but deliberately leaves revocation and reads available, so
  # an operator can contain an incident without losing the tools to resolve it.
  Invoke-Write "Pause the protocol" $protocolId "pause"

  $paused = Invoke-Read "Confirm the pause flag" $protocolId "is_paused"
  if ($paused -notmatch "true") {
    throw "Protocol should report paused, got: $paused"
  }

  Assert-Rejected "Register a proof while paused" $proofId "register_proof" @(
    "--proof_id_hash", $secondProofIdHash,
    "--commitment_hash", $secondCommitment,
    "--issuer_address", $adminAddress,
    "--schema_version", "1",
    "--expires_at", "$expiresAt"
  )

  $stillValid = Invoke-Read "Verification still works while paused" $proofId "is_valid_proof" @(
    "--proof_id_hash", $proofIdHash
  )
  if ($stillValid -notmatch "true") {
    throw "Verification must remain available while paused, got: $stillValid"
  }

  Invoke-Write "Revoke the proof while paused" $proofId "admin_revoke_proof" @(
    "--proof_id_hash", $proofIdHash
  )

  $revoked = Invoke-Read "Confirm the revocation" $proofId "is_revoked" @(
    "--proof_id_hash", $proofIdHash
  )
  if ($revoked -notmatch "true") {
    throw "Proof should report revoked, got: $revoked"
  }

  Invoke-Write "Unpause the protocol" $protocolId "unpause"

  $unpaused = Invoke-Read "Confirm the protocol resumed" $protocolId "is_paused"
  if ($unpaused -notmatch "false") {
    throw "Protocol should report unpaused, got: $unpaused"
  }

  Invoke-Write "Register a proof after unpausing" $proofId "register_proof" @(
    "--proof_id_hash", $secondProofIdHash,
    "--commitment_hash", $secondCommitment,
    "--issuer_address", $adminAddress,
    "--schema_version", "1",
    "--expires_at", "$expiresAt"
  )

  # Revocation must survive the pause lifting.
  $stillRevoked = Invoke-Read "Confirm the revocation survived" $proofId "is_revoked" @(
    "--proof_id_hash", $proofIdHash
  )
  if ($stillRevoked -notmatch "true") {
    throw "Revocation must outlive the pause, got: $stillRevoked"
  }

  # --- Manifest ------------------------------------------------------------
  # Marked disposable in the file itself, so it cannot be mistaken for a
  # deployment record if it is ever pasted somewhere.
  $manifest = [ordered]@{
    network      = "local-sandbox"
    disposable   = $true
    warning      = "Ephemeral local sandbox output. Not a deployment record. Contains no credentials."
    generatedAt  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    admin        = $adminAddress
    identity     = $IdentityName
    contracts    = [ordered]@{
      protocolConfig = $protocolId
      issuerRegistry = $issuerId
      proofRegistry  = $proofId
    }
    wasm         = [ordered]@{
      protocolConfig = [ordered]@{
        path   = "target/wasm32v1-none/release/protocol_config.wasm"
        sha256 = Get-Sha256File $protocolWasm
      }
      issuerRegistry = [ordered]@{
        path   = "target/wasm32v1-none/release/issuer_registry.wasm"
        sha256 = Get-Sha256File $issuerWasm
      }
      proofRegistry  = [ordered]@{
        path   = "target/wasm32v1-none/release/proof_registry.wasm"
        sha256 = Get-Sha256File $proofWasm
      }
    }
    syntheticValues = [ordered]@{
      note              = "Derived from fixed literals; no real wallet, proof, or credential."
      issuerIdHash      = $issuerIdHash
      issuerMetadataHash = $issuerMetadataHash
      proofIdHash       = $proofIdHash
      secondProofIdHash = $secondProofIdHash
    }
    schemaVersions = @(1)
    lifecycle    = @(
      "register_issuer", "is_active_issuer",
      "register_proof", "is_valid_proof",
      "pause", "is_paused",
      "register_proof (rejected while paused)",
      "is_valid_proof (available while paused)",
      "admin_revoke_proof", "is_revoked",
      "unpause", "register_proof", "is_revoked (survives unpause)"
    )
  }

  $outputPath = Join-Path $root $Output
  $outputDir = Split-Path $outputPath -Parent
  if (-not (Test-Path $outputDir)) {
    New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
  }

  $manifest | ConvertTo-Json -Depth 10 | Set-Content -Path $outputPath -Encoding UTF8

  Write-Host ""
  Write-Host "=== Sandbox run complete ===" -ForegroundColor Green
  Write-Host "Contract IDs (safe to share):"
  Write-Host "  protocol-config: $protocolId"
  Write-Host "  issuer-registry: $issuerId"
  Write-Host "  proof-registry:  $proofId"
  Write-Host ""
  Write-Host "Synthetic values (derived from fixed literals, not real data):"
  Write-Host "  issuerIdHash: $issuerIdHash"
  Write-Host "  proofIdHash:  $proofIdHash"
  Write-Host ""
  Write-Host "Disposable manifest: $outputPath"
  Write-Host ""
  Write-Host "No secret key is printed by this script. The sandbox identity is"
  Write-Host "stored by the Stellar CLI under .stellar/, which is gitignored."

  if (-not $KeepState) {
    Write-Host ""
    Write-Host "==> Removing the throwaway identity (pass -KeepState to keep it)"
    & stellar keys rm $IdentityName 2>&1 | Out-Null
  }
}
finally {
  Pop-Location
}
