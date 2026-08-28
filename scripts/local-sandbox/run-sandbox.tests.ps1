#Requires -Version 7.0
<#
.SYNOPSIS
  Smoke test for the local sandbox harness.

.DESCRIPTION
  Validates the harness from a clean environment, in the sense that matters for
  CI: no Docker, no Stellar container, no funded identity.

  What that can and cannot cover is worth stating plainly, because a smoke test
  that overclaims is worse than none.

  COVERED here:
    - the harness parses and its parameters bind;
    - the network guard rejects every non-local network before any side effect;
    - prerequisite failures produce actionable messages rather than stack traces;
    - the lifecycle covers every step the issue requires;
    - no secret is printed and no credential is read from the environment;
    - the manifest path is gitignored.

  NOT covered here:
    - an actual deployment. That needs a running container and is the job of
      the documented manual run in docs/local-development.md.

  The distinction matters: this file proves the harness is safe and structurally
  correct to run, not that a deployment succeeded.

  To run:
    pwsh -NonInteractive -File scripts/local-sandbox/run-sandbox.tests.ps1
#>

$ErrorActionPreference = "Stop"

$ScriptDir = $PSScriptRoot
$Harness = Join-Path $ScriptDir "run-sandbox.ps1"
$RepoRoot = (Resolve-Path (Join-Path $ScriptDir "../..")).Path

$script:PassCount = 0
$script:FailCount = 0
$script:Results = [System.Collections.Generic.List[string]]::new()

function Invoke-Test {
  param([string]$Name, [scriptblock]$Body)

  Write-Host "  RUN  $Name"
  try {
    & $Body
    $script:PassCount++
    $script:Results.Add("  PASS $Name")
    Write-Host "  PASS $Name" -ForegroundColor Green
  }
  catch {
    $script:FailCount++
    $script:Results.Add("  FAIL $Name`n       $_")
    Write-Host "  FAIL $Name`n       $_" -ForegroundColor Red
  }
}

# Runs the harness in a child process and returns exit code plus output.
function Invoke-Harness {
  param([string[]]$HarnessArgs)

  $psi = [System.Diagnostics.ProcessStartInfo]::new()
  $psi.FileName = (Get-Command pwsh -ErrorAction SilentlyContinue)?.Source
  if (-not $psi.FileName) { $psi.FileName = "pwsh" }
  $psi.Arguments = "-NonInteractive -File `"$Harness`" " + ($HarnessArgs -join " ")
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.UseShellExecute = $false
  $psi.WorkingDirectory = $RepoRoot

  $proc = [System.Diagnostics.Process]::Start($psi)
  $stdout = $proc.StandardOutput.ReadToEnd()
  $stderr = $proc.StandardError.ReadToEnd()
  $proc.WaitForExit()

  return @{
    ExitCode = $proc.ExitCode
    Output   = ($stdout + "`n" + $stderr)
  }
}

Write-Host "=== Local sandbox harness smoke test ===" -ForegroundColor Cyan
Write-Host ""

# ---------------------------------------------------------------------------
# Structure
# ---------------------------------------------------------------------------

Invoke-Test "harness script exists" {
  if (-not (Test-Path $Harness)) { throw "Harness not found at $Harness" }
}

Invoke-Test "harness parses without syntax errors" {
  # Parsing separately from execution is what lets this run without Docker.
  $tokens = $null
  $errors = $null
  [System.Management.Automation.Language.Parser]::ParseFile($Harness, [ref]$tokens, [ref]$errors) | Out-Null

  if ($errors -and $errors.Count -gt 0) {
    throw "Parse errors: $($errors | ForEach-Object { $_.Message } | Join-String -Separator '; ')"
  }
}

Invoke-Test "harness declares the expected parameters" {
  $content = Get-Content $Harness -Raw
  foreach ($p in @("Network", "Output", "KeepState", "SkipBuild", "MaxRetries")) {
    if ($content -notmatch "\`$$p") {
      throw "Harness does not declare parameter -$p"
    }
  }
}

# ---------------------------------------------------------------------------
# Safety guards
# ---------------------------------------------------------------------------
# The most important tests in this file. The harness generates throwaway keys
# and deploys unreviewed artifacts; doing that against a shared network would
# be the real failure mode.

Invoke-Test "refuses to run against testnet" {
  $result = Invoke-Harness @("-Network", "testnet")

  if ($result.ExitCode -eq 0) {
    throw "Harness accepted -Network testnet; it must refuse any non-local network."
  }
  if ($result.Output -notmatch "(?i)local sandbox network") {
    throw "Rejection message did not explain the constraint.`n$($result.Output)"
  }
}

Invoke-Test "refuses to run against mainnet" {
  $result = Invoke-Harness @("-Network", "mainnet")

  if ($result.ExitCode -eq 0) {
    throw "Harness accepted -Network mainnet."
  }
}

Invoke-Test "refuses to run against a futurenet or custom network" {
  foreach ($network in @("futurenet", "public", "my-shared-net")) {
    $result = Invoke-Harness @("-Network", $network)
    if ($result.ExitCode -eq 0) {
      throw "Harness accepted -Network $network."
    }
  }
}

Invoke-Test "network guard runs before any side effect" {
  # The rejection must happen before building, deploying, or generating a key.
  $result = Invoke-Harness @("-Network", "testnet")

  foreach ($sideEffect in @("Build contract WASM", "Deploy protocol-config", "Generate sandbox identity")) {
    if ($result.Output -match [regex]::Escape($sideEffect)) {
      throw "Harness performed '$sideEffect' before rejecting a non-local network."
    }
  }
}

Invoke-Test "rejection names the safe alternative" {
  # A guard that blocks without saying what to do instead gets worked around.
  $result = Invoke-Harness @("-Network", "testnet")
  if ($result.Output -notmatch "deploy-testnet\.ps1") {
    throw "Rejection did not point at scripts/deploy-testnet.ps1.`n$($result.Output)"
  }
}

# ---------------------------------------------------------------------------
# Credential hygiene
# ---------------------------------------------------------------------------

Invoke-Test "harness reads no credentials from the environment" {
  $content = Get-Content $Harness -Raw

  # A harness that read a key from the environment could pick up a real one.
  foreach ($pattern in @('\$env:.*SECRET', '\$env:.*PRIVATE', '\$env:.*SEED', '\$env:.*KEY')) {
    if ($content -match $pattern) {
      throw "Harness reads a credential-shaped environment variable: $pattern"
    }
  }
}

Invoke-Test "harness never prints a secret key" {
  $content = Get-Content $Harness -Raw

  if ($content -match "stellar\s+keys\s+show") {
    throw "Harness invokes 'stellar keys show', which prints a secret key."
  }
  if ($content -match '\bS[A-Z2-7]{55}\b') {
    throw "Harness contains a Stellar secret-seed-shaped literal."
  }
}

Invoke-Test "harness contains no hard-coded Stellar addresses" {
  # Every address must be generated at run time. A literal would mean the
  # harness was pointed at something real at some stage.
  $content = Get-Content $Harness -Raw
  $matches = [regex]::Matches($content, '\bG[A-Z2-7]{55}\b')

  if ($matches.Count -gt 0) {
    throw "Harness contains $($matches.Count) hard-coded Stellar address(es)."
  }
}

Invoke-Test "the throwaway identity is obviously disposable" {
  $content = Get-Content $Harness -Raw
  if ($content -notmatch 'IdentityName\s*=\s*"[^"]*throwaway[^"]*"') {
    throw "The sandbox identity name should be self-evidently disposable."
  }
}

# ---------------------------------------------------------------------------
# Lifecycle coverage
# ---------------------------------------------------------------------------

Invoke-Test "harness exercises the full documented lifecycle" {
  $content = Get-Content $Harness -Raw

  # The acceptance criteria name these explicitly.
  $required = @(
    "register_issuer",
    "register_proof",
    "is_valid_proof",
    "admin_revoke_proof",
    "is_revoked",
    "pause",
    "unpause",
    "is_paused"
  )

  foreach ($step in $required) {
    if ($content -notmatch [regex]::Escape($step)) {
      throw "Harness does not exercise '$step'."
    }
  }
}

Invoke-Test "harness deploys all three contracts in dependency order" {
  $content = Get-Content $Harness -Raw

  $protocolAt = $content.IndexOf('Deploy protocol-config')
  $issuerAt = $content.IndexOf('Deploy issuer-registry')
  $proofAt = $content.IndexOf('Deploy proof-registry')

  if ($protocolAt -lt 0 -or $issuerAt -lt 0 -or $proofAt -lt 0) {
    throw "Harness does not deploy all three contracts."
  }

  # proof-registry takes the other two addresses at initialization and has no
  # setter, so it must be deployed last.
  if ($proofAt -lt $protocolAt -or $proofAt -lt $issuerAt) {
    throw "proof-registry must be deployed after its dependencies."
  }
}

Invoke-Test "harness asserts that paused registration is rejected" {
  # Without this, the pause step would deploy and print but prove nothing.
  $content = Get-Content $Harness -Raw
  if ($content -notmatch "Assert-Rejected") {
    throw "Harness does not assert that any operation is rejected."
  }
  if ($content -notmatch "Register a proof while paused") {
    throw "Harness does not check that registration is blocked while paused."
  }
}

Invoke-Test "harness asserts verification stays available while paused" {
  $content = Get-Content $Harness -Raw
  if ($content -notmatch "Verification still works while paused") {
    throw "Harness does not check that reads remain available during a pause."
  }
}

# ---------------------------------------------------------------------------
# Disposability
# ---------------------------------------------------------------------------

Invoke-Test "the default manifest path is gitignored" {
  # A sandbox manifest committed by accident would look like a deployment
  # record. The leading dot keeps it out of the way; the ignore rule keeps it
  # out of git.
  $content = Get-Content $Harness -Raw
  if ($content -notmatch '\.sandbox-manifest\.json') {
    throw "Harness does not use the expected default manifest name."
  }

  $gitignore = Get-Content (Join-Path $RepoRoot ".gitignore") -Raw
  if ($gitignore -notmatch "sandbox-manifest") {
    throw ".gitignore does not exclude the sandbox manifest."
  }
}

Invoke-Test "the manifest is marked disposable in its own body" {
  $content = Get-Content $Harness -Raw
  if ($content -notmatch 'disposable\s*=\s*\$true') {
    throw "Sandbox manifest is not flagged disposable."
  }
  if ($content -notmatch "Not a deployment record") {
    throw "Sandbox manifest does not warn against mistaking it for a deployment record."
  }
}

Invoke-Test "the throwaway identity is removed unless -KeepState is passed" {
  $content = Get-Content $Harness -Raw
  if ($content -notmatch "stellar\s+keys\s+rm") {
    throw "Harness never removes the throwaway identity."
  }
  if ($content -notmatch "KeepState") {
    throw "Harness offers no way to keep sandbox state."
  }
}

# ---------------------------------------------------------------------------
# Documentation
# ---------------------------------------------------------------------------

Invoke-Test "the local development guide exists and covers the harness" {
  $guide = Join-Path $RepoRoot "docs/local-development.md"
  if (-not (Test-Path $guide)) {
    throw "docs/local-development.md is missing."
  }

  $content = Get-Content $guide -Raw
  foreach ($topic in @("run-sandbox.ps1", "stellar container start", "pwsh")) {
    if ($content -notmatch [regex]::Escape($topic)) {
      throw "The guide does not mention '$topic'."
    }
  }
}

Invoke-Test "scripts/README.md links the sandbox harness" {
  $readme = Get-Content (Join-Path $RepoRoot "scripts/README.md") -Raw
  if ($readme -notmatch "local-sandbox") {
    throw "scripts/README.md does not mention the local sandbox harness."
  }
}

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

Write-Host ""
Write-Host "=== Results ===" -ForegroundColor Cyan
foreach ($line in $script:Results) { Write-Host $line }
Write-Host ""
Write-Host "Passed: $($script:PassCount)  Failed: $($script:FailCount)"

if ($script:FailCount -gt 0) {
  exit 1
}
