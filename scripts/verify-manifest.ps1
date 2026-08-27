param(
  [Parameter(Mandatory = $true)]
  [string]$Manifest,

  [switch]$AllowPlaceholders,
  [switch]$Live,
  [string]$CliPath = "stellar",
  [int]$TimeoutSeconds = 30,
  [int]$MaxRetries = 3,
  [string]$Network = "",   # defaults to manifest network if empty

  # Path to a release note under docs/releases/. When supplied, the note is
  # validated against this manifest: required fields must be present, and the
  # declared contract IDs and WASM hashes must match what was deployed.
  # See docs/compatibility.md.
  [string]$Release = ""
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# Offline shape-check helpers
# ---------------------------------------------------------------------------

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

# ---------------------------------------------------------------------------
# Live on-chain helper
# ---------------------------------------------------------------------------

<#
.SYNOPSIS
  Invokes a read-only Stellar contract function and returns the raw stdout.

.DESCRIPTION
  Builds and runs:
    stellar contract invoke --id <ContractId> --network <Network> -- <Function> [<Args>...]

  Retries up to $MaxRetries times on transient errors (timeout, connection
  reset, unavailable).  Throws with a clear message on persistent failure.
#>
function Invoke-StellarRead {
  param(
    [string]$ContractId,
    [string]$Function,
    [string[]]$Args = @(),
    [string]$Network,
    [string]$CliPath,
    [int]$TimeoutSeconds,
    [int]$MaxRetries
  )

  # Build argument list
  $cmdArgs = @(
    "contract", "invoke",
    "--id", $ContractId,
    "--network", $Network,
    "--"
    $Function
  ) + $Args

  $transientPatterns = @(
    "timeout",
    "connection reset",
    "connection refused",
    "temporarily unavailable",
    "send failure",
    "503",
    "502",
    "504"
  )

  $attempt = 0
  while ($true) {
    $attempt++

    # Use temp files so Start-Process can redirect stdout/stderr without
    # blocking the calling thread.  This is what makes the timeout real.
    $stdoutFile = [System.IO.Path]::GetTempFileName()
    $stderrFile = [System.IO.Path]::GetTempFileName()

    try {
      $proc = Start-Process `
        -FilePath $CliPath `
        -ArgumentList $cmdArgs `
        -RedirectStandardOutput $stdoutFile `
        -RedirectStandardError  $stderrFile `
        -NoNewWindow `
        -PassThru

      $finished = $proc.WaitForExit($TimeoutSeconds * 1000)

      if (-not $finished) {
        # Kill the hung process before throwing so it doesn't linger.
        try { $proc.Kill() } catch { }
        throw [System.TimeoutException]::new(
          "stellar CLI timed out after ${TimeoutSeconds}s calling ${Function} on contract ${ContractId}."
        )
      }

      $exitCode = $proc.ExitCode
      $stdout   = (Get-Content $stdoutFile -Raw -ErrorAction SilentlyContinue) ?? ""
      $stderr   = (Get-Content $stderrFile -Raw -ErrorAction SilentlyContinue) ?? ""
      $output   = ($stdout + $stderr).Trim()

      if ($exitCode -ne 0) {
        $isTransient = $false
        foreach ($pattern in $transientPatterns) {
          if ($output -imatch $pattern) {
            $isTransient = $true
            break
          }
        }

        if ($isTransient -and $attempt -lt $MaxRetries) {
          Write-Warning "Transient RPC error on attempt $attempt for ${Function} (contract $ContractId). Retrying..."
          Start-Sleep -Seconds ([math]::Min(2 * $attempt, 10))
          continue
        }

        throw "CLI error calling ${Function} on contract ${ContractId} (exit $exitCode): $output"
      }

      return $stdout.Trim()
    }
    catch [System.TimeoutException] {
      if ($attempt -lt $MaxRetries) {
        Write-Warning "Timeout on attempt $attempt for ${Function} (contract $ContractId). Retrying..."
        Start-Sleep -Seconds ([math]::Min(2 * $attempt, 10))
        continue
      }
      throw "Timed out after $MaxRetries attempt(s) calling ${Function} on contract ${ContractId}."
    }
    finally {
      Remove-Item $stdoutFile -Force -ErrorAction SilentlyContinue
      Remove-Item $stderrFile -Force -ErrorAction SilentlyContinue
    }
  }
}

# ---------------------------------------------------------------------------
# Mismatch reporter — writes structured output and accumulates failures
# ---------------------------------------------------------------------------

$script:LiveFailures = [System.Collections.Generic.List[string]]::new()

function Assert-LiveMatch {
  param(
    [string]$Label,
    [string]$Expected,
    [string]$Actual
  )

  # Strip surrounding quotes that the Stellar CLI often emits for string values
  $cleanActual = $Actual -replace '^"(.*)"$', '$1'

  if ($cleanActual -ne $Expected) {
    $msg = "MISMATCH: $Label`n  expected: $Expected`n  actual:   $cleanActual"
    Write-Host $msg
    $script:LiveFailures.Add($msg)
  }
}

function Assert-LiveCondition {
  param(
    [string]$Label,
    [bool]$Condition,
    [string]$FailMessage
  )

  if (-not $Condition) {
    $msg = "FAIL: $Label — $FailMessage"
    Write-Host $msg
    $script:LiveFailures.Add($msg)
  }
}

# ---------------------------------------------------------------------------
# Offline validation (always runs)
# ---------------------------------------------------------------------------

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

Write-Host "Deployment manifest shape is valid: $path"

# ---------------------------------------------------------------------------
# Release metadata validation (only when -Release is passed)
# ---------------------------------------------------------------------------
# A release note that records a hash is not evidence; the recorded hash has to
# be the hash of the artifact that was actually deployed. This block reconciles
# the note against the manifest and refuses a mismatch.

if ($Release) {
  $releasePath = Resolve-Path $Release
  $releaseText = Get-Content $releasePath -Raw

  # --- Required fields -----------------------------------------------------
  # Kept in step with docs/releases/TEMPLATE.md. A missing section is an
  # incomplete release, not a stylistic difference.
  $requiredSections = @(
    "## Toolchain",
    "## Artifacts",
    "## Changes",
    "## Migration",
    "## Backend compatibility",
    "## Rollback",
    "## Containment",
    "## Governance"
  )

  foreach ($section in $requiredSections) {
    if ($releaseText -notmatch [regex]::Escape($section)) {
      throw "Release note is missing the required section '$section': $releasePath"
    }
  }

  $requiredFields = @(
    @{ Name = "Release";        Pattern = "\*\*Release:\*\*\s*\S+" },
    @{ Name = "Date";           Pattern = "\*\*Date:\*\*\s*\d{4}-\d{2}-\d{2}" },
    @{ Name = "Commit";         Pattern = "\*\*Commit:\*\*\s*[0-9a-f]{40}" },
    @{ Name = "Classification"; Pattern = "\*\*Classification:\*\*\s*(additive|semantic|breaking)" }
  )

  foreach ($field in $requiredFields) {
    if ($releaseText -notmatch $field.Pattern) {
      throw "Release note is missing or malformed field '$($field.Name)': $releasePath"
    }
  }

  # --- Credential scan -----------------------------------------------------
  # A release note is published, and publication is irreversible. The policy in
  # docs/compatibility.md is enforced here rather than left to review.
  if ($releaseText -match "\bS[A-Z2-7]{55}\b") {
    throw "Release note appears to contain a Stellar secret seed: $releasePath"
  }

  $credentialPatterns = @(
    "(?i)(private[_ -]?key|secret[_ -]?key|seed[_ -]?phrase|mnemonic)\s*[:=]\s*\S+",
    "(?i)(api[_ -]?key|access[_ -]?token|bearer)\s*[:=]\s*\S+"
  )

  foreach ($pattern in $credentialPatterns) {
    if ($releaseText -match $pattern) {
      throw "Release note appears to contain credential material: $releasePath"
    }
  }

  # --- Reconciliation against the manifest ---------------------------------
  # The core check. A declared artifact that was not the deployed one makes the
  # whole note misleading, which is worse than having no note at all.
  $contractPairs = @(
    @{ Name = "protocolConfig"; Id = $manifestJson.contracts.protocolConfig; Sha = $manifestJson.wasm.protocolConfig.sha256 },
    @{ Name = "issuerRegistry"; Id = $manifestJson.contracts.issuerRegistry; Sha = $manifestJson.wasm.issuerRegistry.sha256 },
    @{ Name = "proofRegistry";  Id = $manifestJson.contracts.proofRegistry;  Sha = $manifestJson.wasm.proofRegistry.sha256 }
  )

  foreach ($pair in $contractPairs) {
    if ($pair.Id -and -not $AllowPlaceholders) {
      if ($releaseText -notmatch [regex]::Escape($pair.Id)) {
        throw "Release note does not record the deployed $($pair.Name) contract ID $($pair.Id): $releasePath"
      }
    }

    if ($pair.Sha -and -not $AllowPlaceholders) {
      if ($releaseText -notmatch [regex]::Escape($pair.Sha)) {
        throw "Release note does not record the deployed $($pair.Name) WASM hash $($pair.Sha): $releasePath"
      }
    }
  }

  # A hash present in the note but absent from the manifest means the note
  # describes an artifact this deployment does not contain.
  $manifestHashes = @($contractPairs | ForEach-Object { $_.Sha } | Where-Object { $_ })
  $noteHashes = [regex]::Matches($releaseText, "\b[a-f0-9]{64}\b") |
    ForEach-Object { $_.Value } |
    Select-Object -Unique

  foreach ($noteHash in $noteHashes) {
    if ($manifestHashes -notcontains $noteHash) {
      throw "Release note records WASM hash $noteHash, which is not in the manifest: $releasePath"
    }
  }

  # --- Breaking-change governance ------------------------------------------
  # docs/compatibility.md requires four things before a breaking artifact ships.
  # Enforced here so the requirement is not merely stated.
  if ($releaseText -match "\*\*Classification:\*\*\s*breaking") {
    if ($releaseText -notmatch "(?i)\*\*Breaking change approved by:\*\*\s*(?!not required)\S+") {
      throw "A breaking release must name the approving maintainer: $releasePath"
    }

    foreach ($required in @("## Migration", "## Rollback", "## Containment")) {
      $body = ($releaseText -split [regex]::Escape($required))[1]
      if (-not $body) {
        throw "A breaking release must document '$required': $releasePath"
      }

      $firstParagraph = ($body -split "`n##")[0].Trim()
      if ($firstParagraph.Length -lt 20) {
        throw "A breaking release needs substantive content under '$required': $releasePath"
      }
    }
  }

  Write-Host "Release metadata matches the manifest: $releasePath"
}

# ---------------------------------------------------------------------------
# Live on-chain checks (only when -Live is passed)
# ---------------------------------------------------------------------------

if ($Live) {
  # Resolve which network to use
  $resolvedNetwork = if ($Network -ne "") { $Network } else { $manifestJson.network }

  $adminAddress        = $manifestJson.admin
  $protocolConfigId    = $manifestJson.contracts.protocolConfig
  $issuerRegistryId    = $manifestJson.contracts.issuerRegistry
  $proofRegistryId     = $manifestJson.contracts.proofRegistry

  $liveParams = @{
    Network        = $resolvedNetwork
    CliPath        = $CliPath
    TimeoutSeconds = $TimeoutSeconds
    MaxRetries     = $MaxRetries
  }

  Write-Host ""
  Write-Host "Running live on-chain checks against network: $resolvedNetwork"
  Write-Host "---"

  # -- protocolConfig --------------------------------------------------------

  Write-Host "Checking protocolConfig admin..."
  $pcAdmin = Invoke-StellarRead -ContractId $protocolConfigId -Function "get_admin" @liveParams
  Assert-LiveMatch "protocolConfig admin" $adminAddress $pcAdmin

  Write-Host "Checking protocolConfig is_paused..."
  $pcPaused = Invoke-StellarRead -ContractId $protocolConfigId -Function "is_paused" @liveParams
  $pausedBool = ($pcPaused.Trim() -ieq "true")
  Assert-LiveCondition "protocolConfig is_paused" (-not $pausedBool) "contract reports paused=true (expected false)"

  Write-Host "Checking protocolConfig get_config_version..."
  $pcVersion = Invoke-StellarRead -ContractId $protocolConfigId -Function "get_config_version" @liveParams
  $versionInt = 0
  $parsedOk = [int]::TryParse($pcVersion.Trim(), [ref]$versionInt)
  if (-not $parsedOk) {
    throw "Malformed output from get_config_version — expected integer, got: $pcVersion"
  }
  Assert-LiveCondition "protocolConfig get_config_version" ($versionInt -gt 0) "config version must be a positive integer, got: $versionInt"

  Write-Host "Checking protocolConfig schema version approvals..."
  foreach ($ver in $manifestJson.schemaVersions) {
    $approved = Invoke-StellarRead -ContractId $protocolConfigId -Function "is_schema_approved" -Args @("--version", "$ver") @liveParams
    $approvedBool = ($approved.Trim() -ieq "true")
    Assert-LiveCondition "protocolConfig schema version $ver approved" $approvedBool "is_schema_approved returned false for version $ver"
  }

  # -- issuerRegistry --------------------------------------------------------

  Write-Host "Checking issuerRegistry admin..."
  $irAdmin = Invoke-StellarRead -ContractId $issuerRegistryId -Function "get_admin" @liveParams
  Assert-LiveMatch "issuerRegistry admin" $adminAddress $irAdmin

  if ($manifestJson.initialIssuer) {
    $issuerAddr = $manifestJson.initialIssuer.address
    Write-Host "Checking issuerRegistry issuer status for $issuerAddr..."
    $issuerStatus = Invoke-StellarRead -ContractId $issuerRegistryId -Function "get_issuer_status" -Args @("--address", $issuerAddr) @liveParams
    $cleanStatus = $issuerStatus.Trim() -replace '^"(.*)"$', '$1'
    Assert-LiveCondition "issuerRegistry initialIssuer status" ($cleanStatus -ne "NotFound") "get_issuer_status returned NotFound for $issuerAddr"
  }

  # -- proofRegistry ---------------------------------------------------------

  Write-Host "Checking proofRegistry admin..."
  $prAdmin = Invoke-StellarRead -ContractId $proofRegistryId -Function "get_admin" @liveParams
  Assert-LiveMatch "proofRegistry admin" $adminAddress $prAdmin

  Write-Host "Checking proofRegistry get_issuer_registry..."
  $prIssuerReg = Invoke-StellarRead -ContractId $proofRegistryId -Function "get_issuer_registry" @liveParams
  Assert-LiveMatch "proofRegistry issuerRegistry reference" $issuerRegistryId $prIssuerReg

  Write-Host "Checking proofRegistry get_protocol_config..."
  $prProtocolCfg = Invoke-StellarRead -ContractId $proofRegistryId -Function "get_protocol_config" @liveParams
  Assert-LiveMatch "proofRegistry protocolConfig reference" $protocolConfigId $prProtocolCfg

  Write-Host "---"

  if ($script:LiveFailures.Count -gt 0) {
    Write-Host ""
    Write-Host "$($script:LiveFailures.Count) live check(s) failed." -ForegroundColor Red
    exit 1
  }

  Write-Host "All live on-chain checks passed." -ForegroundColor Green
}
