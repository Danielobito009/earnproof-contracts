param(
  [Parameter(Mandatory = $true)]
  [string]$Manifest,

  [switch]$AllowPlaceholders,
  [switch]$Live,
  [string]$CliPath = "stellar",
  [int]$TimeoutSeconds = 30,
  [int]$MaxRetries = 3,
  [string]$Network = ""   # defaults to manifest network if empty
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
