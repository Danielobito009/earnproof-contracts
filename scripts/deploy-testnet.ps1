param(
  [Parameter(Mandatory = $true)]
  [string]$Source,

  [Parameter(Mandatory = $true)]
  [string]$Admin,

  [string]$Network = "testnet",
  [string]$Output = "scripts/deployment-manifest.testnet.json"
)

$ErrorActionPreference = "Stop"

function Assert-Command($Name) {
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Required command '$Name' was not found."
  }
}

function Invoke-Step($Description, $Command) {
  Write-Host "==> $Description"
  & $Command[0] @($Command | Select-Object -Skip 1)
  if ($LASTEXITCODE -ne 0) {
    throw "Command failed: $($Command -join ' ')"
  }
}

function Invoke-Capture($Description, $Command) {
  Write-Host "==> $Description"
  $result = & $Command[0] @($Command | Select-Object -Skip 1)
  if ($LASTEXITCODE -ne 0) {
    throw "Command failed: $($Command -join ' ')"
  }
  return ($result | Select-Object -Last 1).Trim()
}

function Get-Sha256($Path) {
  return (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

Assert-Command "cargo"
Assert-Command "stellar"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $root

try {
  Invoke-Step "Build contract WASM artifacts" @("cargo", "build", "--workspace", "--target", "wasm32-unknown-unknown", "--release")

  $wasmRoot = Join-Path $root "target/wasm32-unknown-unknown/release"
  $protocolWasm = Join-Path $wasmRoot "protocol_config.wasm"
  $issuerWasm = Join-Path $wasmRoot "issuer_registry.wasm"
  $proofWasm = Join-Path $wasmRoot "proof_registry.wasm"

  foreach ($wasm in @($protocolWasm, $issuerWasm, $proofWasm)) {
    if (-not (Test-Path $wasm)) {
      throw "Expected WASM artifact was not found: $wasm"
    }
  }

  $protocolId = Invoke-Capture "Deploy protocol-config" @("stellar", "contract", "deploy", "--source", $Source, "--network", $Network, "--wasm", $protocolWasm)
  $issuerId = Invoke-Capture "Deploy issuer-registry" @("stellar", "contract", "deploy", "--source", $Source, "--network", $Network, "--wasm", $issuerWasm)
  $proofId = Invoke-Capture "Deploy proof-registry" @("stellar", "contract", "deploy", "--source", $Source, "--network", $Network, "--wasm", $proofWasm)

  Invoke-Step "Initialize protocol-config" @("stellar", "contract", "invoke", "--source", $Source, "--network", $Network, "--id", $protocolId, "--", "initialize", "--admin", $Admin)
  Invoke-Step "Approve schema version 1" @("stellar", "contract", "invoke", "--source", $Source, "--network", $Network, "--id", $protocolId, "--", "approve_schema_version", "--version", "1")
  Invoke-Step "Initialize issuer-registry" @("stellar", "contract", "invoke", "--source", $Source, "--network", $Network, "--id", $issuerId, "--", "initialize", "--admin", $Admin)
  Invoke-Step "Initialize proof-registry" @("stellar", "contract", "invoke", "--source", $Source, "--network", $Network, "--id", $proofId, "--", "initialize", "--admin", $Admin, "--issuer_registry", $issuerId, "--protocol_config", $protocolId)

  $manifest = [ordered]@{
    network = "stellar-$Network"
    deployedAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    admin = $Admin
    source = $Source
    contracts = [ordered]@{
      protocolConfig = $protocolId
      issuerRegistry = $issuerId
      proofRegistry = $proofId
    }
    wasm = [ordered]@{
      protocolConfig = [ordered]@{
        path = "target/wasm32-unknown-unknown/release/protocol_config.wasm"
        sha256 = Get-Sha256 $protocolWasm
      }
      issuerRegistry = [ordered]@{
        path = "target/wasm32-unknown-unknown/release/issuer_registry.wasm"
        sha256 = Get-Sha256 $issuerWasm
      }
      proofRegistry = [ordered]@{
        path = "target/wasm32-unknown-unknown/release/proof_registry.wasm"
        sha256 = Get-Sha256 $proofWasm
      }
    }
    schemaVersions = @(1)
    commands = @(
      "cargo build --workspace --target wasm32-unknown-unknown --release",
      "stellar contract deploy --source <source> --network $Network --wasm <wasm>",
      "stellar contract invoke --source <source> --network $Network --id <contract> -- <function>"
    )
  }

  $outputPath = Join-Path $root $Output
  $manifest | ConvertTo-Json -Depth 10 | Set-Content -Path $outputPath -Encoding UTF8
  Write-Host "Wrote deployment manifest: $outputPath"
}
finally {
  Pop-Location
}
