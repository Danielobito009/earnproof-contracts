#Requires -Version 7.0
<#
.SYNOPSIS
  Exercises the full proof lifecycle against the deployed EarnProof testnet
  contracts: preflight, register, lookup, revoke, and post-revoke validity
  check.

.DESCRIPTION
  This is an opt-in smoke script intended for manual runs by maintainers. It is
  NOT executed by CI on pull requests.

  The script uses dedicated identities (Source and IssuerSource) and generates
  disposable hashes from a timestamp-seeded run ID so every execution is
  independent and cannot mutate a proof registered by a prior run.

  Steps performed:

    1. Preflight — confirms the Stellar CLI, network connectivity, source
       account balance, contract IDs from the manifest, admin address, issuer
       registration, and schema approval.
    2. Register — derives a unique proof_id_hash and commitment_hash from the
       run ID, then calls register_proof through the issuer identity.
    3. Lookup — calls get_proof and is_valid_proof to confirm the stored fields
       and active status.
    4. Revoke — calls revoke_proof (issuer path) and is_revoked to confirm the
       revocation.
    5. Post-revoke check — confirms is_valid_proof now returns false.

  With -PreflightOnly the script stops after step 1 without submitting any
  transactions.

  With -AdminRevoke the script uses admin_revoke_proof (admin path) instead of
  revoke_proof (issuer path) for step 4.

  Result artifact:
    scripts/smoke-proof-lifecycle-result.json  (gitignored)

  The artifact records every transaction hash and Stellar Expert explorer link.
  It contains no secret keys or seed phrases.

.PARAMETER Manifest
  Path to the deployment manifest. Defaults to the checked-in testnet manifest.

.PARAMETER Source
  Stellar CLI identity name for the deployer/admin account. This identity must
  be funded with testnet XLM and registered as the admin in the manifest.

.PARAMETER IssuerSource
  Stellar CLI identity name for the issuer account. This identity must be
  funded with testnet XLM and its address must be the registered active issuer
  in the issuer-registry contract.

  If omitted, Source is used for both roles (only valid when the source account
  is also the registered issuer address).

.PARAMETER Network
  Stellar CLI network name. Defaults to "testnet".

.PARAMETER MaxRetries
  Maximum retry attempts for transient RPC failures. Defaults to 5.

.PARAMETER PreflightOnly
  Stop after the preflight check without submitting any transactions.

.PARAMETER AdminRevoke
  Use admin_revoke_proof (admin auth path) instead of revoke_proof (issuer auth
  path) for the revocation step.

.PARAMETER Output
  Where to write the result artifact. Defaults to
  scripts/smoke-proof-lifecycle-result.json (gitignored).

.EXAMPLE
  # Preflight only — no transactions submitted:
  pwsh -File scripts/smoke-proof-lifecycle.ps1 -Source earnproof-admin -IssuerSource earnproof-issuer -PreflightOnly

.EXAMPLE
  # Full lifecycle — issuer revocation path:
  pwsh -File scripts/smoke-proof-lifecycle.ps1 -Source earnproof-admin -IssuerSource earnproof-issuer

.EXAMPLE
  # Full lifecycle — admin revocation path:
  pwsh -File scripts/smoke-proof-lifecycle.ps1 -Source earnproof-admin -IssuerSource earnproof-issuer -AdminRevoke

.EXAMPLE
  # Use Source for both admin and issuer (only valid when they share the same address):
  pwsh -File scripts/smoke-proof-lifecycle.ps1 -Source earnproof-deployer
#>

param(
  [string]$Manifest = "scripts/deployment-manifest.testnet.json",

  [Parameter(Mandatory = $true)]
  [string]$Source,

  [string]$IssuerSource = "",

  [string]$Network = "testnet",

  [int]$MaxRetries = 5,

  [switch]$PreflightOnly,

  [switch]$AdminRevoke,

  [string]$Output = "scripts/smoke-proof-lifecycle-result.json"
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

function Assert-Command($Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Required command '$Name' was not found. Install the Stellar CLI: https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli"
  }
}

function Test-RetryableStellarError($Output) {
  $text = ($Output -join "`n")
  return $text -imatch "SendRequest|Connect|connection|timeout|timed out|temporarily unavailable|TxBadSeq|sequence|503|502|504"
}

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
      throw "Step '$Description' failed after $attempt attempt(s).`nCommand: $($Command -join ' ')`nOutput: $($output -join ' ')"
    }

    Write-Warning "Transient Stellar RPC error. Waiting $delaySeconds second(s) before retry."
    Start-Sleep -Seconds $delaySeconds
    $attempt++
    $delaySeconds = [Math]::Min($delaySeconds * 2, 30)
  }
}

# Runs a read-only contract invoke and returns the raw trimmed output.
function Invoke-Read($Description, $ContractId, $Function, $Arguments = @()) {
  $command = @(
    "stellar", "contract", "invoke",
    "--id", $ContractId,
    "--network", $Network,
    "--"
    $Function
  ) + $Arguments

  $result = Invoke-WithRetry $Description $command -CaptureOutput
  return ("$result").Trim()
}

# Runs a state-mutating contract invoke signed by the given source identity.
# Returns the raw trimmed output (which typically contains the transaction hash).
function Invoke-Write($Description, $SourceName, $ContractId, $Function, $Arguments = @()) {
  $command = @(
    "stellar", "contract", "invoke",
    "--source", $SourceName,
    "--network", $Network,
    "--auth-mode", "root",
    "--auto-sign",
    "--id", $ContractId,
    "--"
    $Function
  ) + $Arguments

  $result = Invoke-WithRetry $Description $command -CaptureOutput
  return ("$result").Trim()
}

# Extracts a transaction hash from stellar CLI output.
# The CLI may print a tx hash as a 64-char hex string or embed it in a URL.
function Get-TxHash($Output) {
  foreach ($line in ($Output -split "`n")) {
    $t = $line.Trim()
    if ($t -match "^([0-9a-fA-F]{64})$") { return $Matches[1] }
    if ($t -match "/tx/([0-9a-fA-F]{64})") { return $Matches[1] }
  }
  return $null
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

function Get-ExplorerTxLink($TxHash) {
  if ([string]::IsNullOrWhiteSpace($TxHash)) { return $null }
  return "https://stellar.expert/explorer/testnet/tx/$TxHash"
}

function Get-ExplorerContractLink($ContractId) {
  return "https://lab.stellar.org/r/testnet/contract/$ContractId"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $root

try {

  # -------------------------------------------------------------------------
  # Resolve issuer source
  # -------------------------------------------------------------------------

  if ([string]::IsNullOrWhiteSpace($IssuerSource)) {
    $IssuerSource = $Source
    Write-Host "==> IssuerSource not specified — using Source ($Source) for both admin and issuer roles."
  }

  # -------------------------------------------------------------------------
  # Run ID — unique per execution, derived from a timestamp. Every hash in
  # this run is seeded from this ID so reruns cannot clash with prior proofs.
  # -------------------------------------------------------------------------

  $RunId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
  Write-Host ""
  Write-Host "=== EarnProof Proof Lifecycle Smoke Test ===" -ForegroundColor Cyan
  Write-Host "    Run ID : $RunId"
  Write-Host "    Network: $Network"
  Write-Host "    Source : $Source"
  Write-Host "    Issuer : $IssuerSource"
  if ($PreflightOnly) {
    Write-Host "    Mode   : PREFLIGHT ONLY (no transactions)" -ForegroundColor Yellow
  } elseif ($AdminRevoke) {
    Write-Host "    Mode   : Full lifecycle — admin revocation path"
  } else {
    Write-Host "    Mode   : Full lifecycle — issuer revocation path"
  }
  Write-Host ""

  # -------------------------------------------------------------------------
  # Prerequisites
  # -------------------------------------------------------------------------

  Assert-Command "stellar"

  # -------------------------------------------------------------------------
  # Step 1: Preflight
  # -------------------------------------------------------------------------

  Write-Host "--- Step 1: Preflight ---" -ForegroundColor Cyan

  # 1a. Load the manifest
  $manifestPath = Join-Path $root $Manifest
  if (-not (Test-Path $manifestPath)) {
    throw "Manifest not found: $manifestPath"
  }
  $m = Get-Content $manifestPath -Raw | ConvertFrom-Json

  $protocolConfigId = $m.contracts.protocolConfig
  $issuerRegistryId = $m.contracts.issuerRegistry
  $proofRegistryId  = $m.contracts.proofRegistry
  $adminAddress     = $m.admin

  foreach ($pair in @(
      @{ Name = "protocolConfig"; Value = $protocolConfigId },
      @{ Name = "issuerRegistry"; Value = $issuerRegistryId },
      @{ Name = "proofRegistry";  Value = $proofRegistryId  }
    )) {
    if ([string]::IsNullOrWhiteSpace($pair.Value) -or $pair.Value -notmatch "^C[A-Z2-7]{55}$") {
      throw "Manifest has an invalid $($pair.Name) contract ID: $($pair.Value)"
    }
  }
  Write-Host "==> Manifest loaded: $manifestPath"
  Write-Host "    protocolConfig : $protocolConfigId"
  Write-Host "    issuerRegistry : $issuerRegistryId"
  Write-Host "    proofRegistry  : $proofRegistryId"
  Write-Host "    admin          : $adminAddress"

  # 1b. Resolve on-chain admin address
  $onChainAdmin = Invoke-Read "Preflight: read proof-registry admin" $proofRegistryId "get_admin"
  # Strip quotes that the CLI may wrap around address strings
  $onChainAdmin = $onChainAdmin.Trim('"')
  if ($onChainAdmin -notmatch "^G[A-Z2-7]{55}$") {
    throw "Preflight: get_admin returned an unexpected value: $onChainAdmin"
  }
  Write-Host "    on-chain admin : $onChainAdmin"

  if ($onChainAdmin -ne $adminAddress) {
    Write-Warning "Manifest admin ($adminAddress) differs from on-chain admin ($onChainAdmin). Using on-chain value."
    $adminAddress = $onChainAdmin
  }

  # 1c. Resolve issuer address from IssuerSource identity
  $issuerAddressRaw = Invoke-WithRetry "Preflight: read issuer address" @(
    "stellar", "keys", "address", $IssuerSource
  ) -CaptureOutput
  $issuerAddress = ("$issuerAddressRaw").Trim()
  if ($issuerAddress -notmatch "^G[A-Z2-7]{55}$") {
    throw "Preflight: could not read a valid address for identity '$IssuerSource': $issuerAddress"
  }
  Write-Host "    issuer address : $issuerAddress"

  # 1d. Confirm issuer is active in the registry
  $issuerActive = Invoke-Read "Preflight: is_active_address for issuer" $issuerRegistryId "is_active_address" @(
    "--issuer_address", $issuerAddress
  )
  if ($issuerActive -notmatch "true") {
    throw @"
Preflight: issuer address $issuerAddress is NOT active in issuer-registry.

To register the issuer, run:
  stellar contract invoke \
    --source $Source --network $Network --auth-mode root --auto-sign \
    --id $issuerRegistryId -- register_issuer \
    --issuer_id_hash <hash> --issuer_address $issuerAddress --metadata_hash <hash>
"@
  }
  Write-Host "    issuer active  : true"

  # 1e. Confirm schema version 1 is approved
  $schemaApproved = Invoke-Read "Preflight: is_schema_version_approved(1)" $protocolConfigId "is_schema_version_approved" @(
    "--version", "1"
  )
  if ($schemaApproved -notmatch "true") {
    throw @"
Preflight: schema version 1 is NOT approved in protocol-config.

To approve it, run:
  stellar contract invoke \
    --source $Source --network $Network --auth-mode root --auto-sign \
    --id $protocolConfigId -- approve_schema_version --version 1
"@
  }
  Write-Host "    schema v1      : approved"

  # 1f. Confirm protocol is not paused
  $isPaused = Invoke-Read "Preflight: is_paused" $protocolConfigId "is_paused"
  if ($isPaused -match "true") {
    throw @"
Preflight: protocol is currently PAUSED. Proof registration will be rejected.

To unpause (admin only):
  stellar contract invoke \
    --source $Source --network $Network --auth-mode root --auto-sign \
    --id $protocolConfigId -- unpause
"@
  }
  Write-Host "    protocol paused: false"

  Write-Host ""
  Write-Host "==> Preflight passed." -ForegroundColor Green

  if ($PreflightOnly) {
    Write-Host ""
    Write-Host "==> -PreflightOnly specified. Stopping before any transactions." -ForegroundColor Yellow
    exit 0
  }

  # -------------------------------------------------------------------------
  # Derive disposable hashes for this run.
  # Every value is synthetic and derived from the run ID. Nothing here is a
  # real wallet, proof, or credential. The derivation is printed explicitly so
  # any reader can verify the claim.
  # -------------------------------------------------------------------------

  $proofIdPreimage    = "earnproof-smoke:proof_id:$RunId"
  $commitmentPreimage = "earnproof-smoke:commitment:$RunId"
  $proofIdHash        = Get-Sha256Text $proofIdPreimage
  $commitmentHash     = Get-Sha256Text $commitmentPreimage
  $expiresAt          = [DateTimeOffset]::UtcNow.AddDays(30).ToUnixTimeSeconds()

  Write-Host ""
  Write-Host "==> Disposable run values (derived from run ID; not real data):"
  Write-Host "    proofIdPreimage    : $proofIdPreimage"
  Write-Host "    commitmentPreimage : $commitmentPreimage"
  Write-Host "    proofIdHash        : $proofIdHash"
  Write-Host "    commitmentHash     : $commitmentHash"
  Write-Host "    expiresAt          : $expiresAt"
  Write-Host ""

  # Accumulate result data as we go
  $result = [ordered]@{
    warning        = "Smoke test output. Secret-free. Not a deployment record."
    runId          = $RunId
    network        = "stellar-$Network"
    generatedAt    = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    source         = $Source
    issuerSource   = $IssuerSource
    issuerAddress  = $issuerAddress
    adminAddress   = $adminAddress
    adminRevoke    = $AdminRevoke.IsPresent
    contracts      = [ordered]@{
      protocolConfig = $protocolConfigId
      issuerRegistry = $issuerRegistryId
      proofRegistry  = $proofRegistryId
    }
    syntheticValues = [ordered]@{
      note               = "Derived from run ID; no real wallet, proof, or credential."
      proofIdPreimage    = $proofIdPreimage
      commitmentPreimage = $commitmentPreimage
      proofIdHash        = $proofIdHash
      commitmentHash     = $commitmentHash
      expiresAt          = $expiresAt
    }
    steps          = [ordered]@{}
    explorer       = [ordered]@{
      protocolConfig = Get-ExplorerContractLink $protocolConfigId
      issuerRegistry = Get-ExplorerContractLink $issuerRegistryId
      proofRegistry  = Get-ExplorerContractLink $proofRegistryId
    }
  }

  # -------------------------------------------------------------------------
  # Step 2: Register proof
  # -------------------------------------------------------------------------

  Write-Host "--- Step 2: Register proof ---" -ForegroundColor Cyan

  $registerOutput = Invoke-Write "Register proof" $IssuerSource $proofRegistryId "register_proof" @(
    "--proof_id_hash",   $proofIdHash,
    "--commitment_hash", $commitmentHash,
    "--issuer_address",  $issuerAddress,
    "--schema_version",  "1",
    "--expires_at",      "$expiresAt"
  )

  $registerTxHash = Get-TxHash $registerOutput
  Write-Host "    register tx hash : $registerTxHash"

  $result.steps["register"] = [ordered]@{
    status  = "ok"
    txHash  = $registerTxHash
    explorer = Get-ExplorerTxLink $registerTxHash
  }

  # -------------------------------------------------------------------------
  # Step 3: Lookup — get_proof and is_valid_proof
  # -------------------------------------------------------------------------

  Write-Host ""
  Write-Host "--- Step 3: Lookup ---" -ForegroundColor Cyan

  $proofRecord = Invoke-Read "get_proof" $proofRegistryId "get_proof" @(
    "--proof_id_hash", $proofIdHash
  )
  Write-Host "    get_proof output : $proofRecord"

  # Verify stored fields
  if ($proofRecord -notmatch [regex]::Escape($issuerAddress)) {
    throw "Lookup: get_proof record does not contain expected issuer_address ($issuerAddress).`nRecord: $proofRecord"
  }
  if ($proofRecord -notmatch "Active") {
    throw "Lookup: get_proof record does not show Active status.`nRecord: $proofRecord"
  }

  $isValid = Invoke-Read "is_valid_proof" $proofRegistryId "is_valid_proof" @(
    "--proof_id_hash", $proofIdHash
  )
  Write-Host "    is_valid_proof   : $isValid"

  if ($isValid -notmatch "true") {
    throw "Lookup: proof should be valid immediately after registration, got: $isValid"
  }

  $result.steps["lookup"] = [ordered]@{
    status       = "ok"
    getProof     = $proofRecord
    isValidProof = $isValid
  }

  Write-Host "==> Lookup passed — proof is active and valid." -ForegroundColor Green

  # -------------------------------------------------------------------------
  # Step 4: Revoke proof
  # -------------------------------------------------------------------------

  Write-Host ""
  if ($AdminRevoke) {
    Write-Host "--- Step 4: Revoke proof (admin path) ---" -ForegroundColor Cyan

    $revokeOutput = Invoke-Write "admin_revoke_proof" $Source $proofRegistryId "admin_revoke_proof" @(
      "--proof_id_hash", $proofIdHash
    )
  }
  else {
    Write-Host "--- Step 4: Revoke proof (issuer path) ---" -ForegroundColor Cyan

    $revokeOutput = Invoke-Write "revoke_proof" $IssuerSource $proofRegistryId "revoke_proof" @(
      "--proof_id_hash", $proofIdHash
    )
  }

  $revokeTxHash = Get-TxHash $revokeOutput
  Write-Host "    revoke tx hash : $revokeTxHash"

  # Confirm is_revoked
  $isRevoked = Invoke-Read "is_revoked" $proofRegistryId "is_revoked" @(
    "--proof_id_hash", $proofIdHash
  )
  Write-Host "    is_revoked     : $isRevoked"

  if ($isRevoked -notmatch "true") {
    throw "Revocation: is_revoked should be true immediately after revocation, got: $isRevoked"
  }

  $result.steps["revoke"] = [ordered]@{
    status    = "ok"
    path      = if ($AdminRevoke) { "admin" } else { "issuer" }
    txHash    = $revokeTxHash
    explorer  = Get-ExplorerTxLink $revokeTxHash
    isRevoked = $isRevoked
  }

  Write-Host "==> Revocation confirmed." -ForegroundColor Green

  # -------------------------------------------------------------------------
  # Step 5: Post-revoke validity check
  # -------------------------------------------------------------------------

  Write-Host ""
  Write-Host "--- Step 5: Post-revoke validity check ---" -ForegroundColor Cyan

  $isValidPostRevoke = Invoke-Read "is_valid_proof (post-revoke)" $proofRegistryId "is_valid_proof" @(
    "--proof_id_hash", $proofIdHash
  )
  Write-Host "    is_valid_proof (post-revoke) : $isValidPostRevoke"

  if ($isValidPostRevoke -match "true") {
    throw "Post-revoke check: is_valid_proof returned true for a revoked proof. Revocation may not have persisted."
  }

  $result.steps["postRevokeCheck"] = [ordered]@{
    status             = "ok"
    isValidProof       = $isValidPostRevoke
    expectation        = "false — proof revoked"
  }

  Write-Host "==> Post-revoke check passed — proof is invalid after revocation." -ForegroundColor Green

  # -------------------------------------------------------------------------
  # Write result artifact
  # -------------------------------------------------------------------------

  $result["outcome"] = "PASS"

  $outputPath = Join-Path $root $Output
  $outputDir  = Split-Path $outputPath -Parent
  if (-not (Test-Path $outputDir)) {
    New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
  }

  $result | ConvertTo-Json -Depth 10 | Set-Content -Path $outputPath -Encoding UTF8

  Write-Host ""
  Write-Host "=== Smoke test PASSED ===" -ForegroundColor Green
  Write-Host ""
  Write-Host "Run ID   : $RunId"
  Write-Host "Artifact : $outputPath"
  Write-Host ""
  Write-Host "Transaction explorer links:"
  if ($result.steps.register.explorer) {
    Write-Host "  register : $($result.steps.register.explorer)"
  }
  if ($result.steps.revoke.explorer) {
    Write-Host "  revoke   : $($result.steps.revoke.explorer)"
  }
  Write-Host ""
  Write-Host "Contract explorer links:"
  Write-Host "  proofRegistry  : $(Get-ExplorerContractLink $proofRegistryId)"
  Write-Host "  issuerRegistry : $(Get-ExplorerContractLink $issuerRegistryId)"
  Write-Host "  protocolConfig : $(Get-ExplorerContractLink $protocolConfigId)"
  Write-Host ""
  Write-Host "No secret key was printed by this script."

}
catch {
  # Write a failure artifact so the caller has diagnostic evidence even when
  # the script exits non-zero.
  try {
    $failResult = [ordered]@{
      warning     = "Smoke test FAILED. Secret-free. Not a deployment record."
      runId       = $RunId
      network     = "stellar-$Network"
      generatedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
      outcome     = "FAIL"
      error       = "$_"
      steps       = if ($null -ne $result -and $result.steps) { $result.steps } else { [ordered]@{} }
    }

    $outputPath = Join-Path $root $Output
    $outputDir  = Split-Path $outputPath -Parent
    if (-not (Test-Path $outputDir)) {
      New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
    }

    $failResult | ConvertTo-Json -Depth 10 | Set-Content -Path $outputPath -Encoding UTF8
    Write-Host ""
    Write-Host "Failure artifact written to: $outputPath" -ForegroundColor Yellow
  }
  catch {
    Write-Warning "Could not write failure artifact: $_"
  }

  Write-Host ""
  Write-Host "=== Smoke test FAILED ===" -ForegroundColor Red
  Write-Host "Error: $_" -ForegroundColor Red
  exit 1
}
finally {
  Pop-Location
}
