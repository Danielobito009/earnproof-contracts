#Requires -Version 7.0
<#
.SYNOPSIS
  Tests for verify-manifest.ps1 — covers both offline shape checks and live
  on-chain checks via a mock Invoke-StellarRead.

.DESCRIPTION
  Runs without Pester.  Each test is a plain PowerShell function that throws
  on failure.  The harness at the bottom runs every test_, reports pass/fail,
  and exits non-zero if any test failed.

  To run:
    pwsh -NonInteractive -File scripts\verify-manifest.tests.ps1

  Pester is optional.  If it is installed the file still works as-is because
  test_ functions are invoked directly — Pester discovery is not required.
#>

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# Paths and shared fixtures
# ---------------------------------------------------------------------------

$ScriptDir     = $PSScriptRoot
$VerifyScript  = Join-Path $ScriptDir "verify-manifest.ps1"
$TestnetManifest = Join-Path $ScriptDir "deployment-manifest.testnet.json"

# Constants that match the testnet manifest
$ADMIN       = "GCDPMNCCMADKEL4YJAJNJXTCGZFAGWQCEFXJYBZVLJCFYOI76FTX6HMV"
$PC_ID       = "CC3OREX5QBIKJ5JOW36JFJJW7TLAKJOVT5WJXEITGALO7MU32KHICS2A"
$IR_ID       = "CB73TVWVJIIVNTKLWSHZB5NL2UIF3B3EUL4YH4MUD6EYX6SFIHE77D2F"
$PR_ID       = "CCMTAXBWN2ZGEDVKGHT6GQENZSTBSLQAGYGGKJWNMDSTVRT2QNMMNWRK"
$ISSUER_ADDR = "GCDPMNCCMADKEL4YJAJNJXTCGZFAGWQCEFXJYBZVLJCFYOI76FTX6HMV"

# ---------------------------------------------------------------------------
# Test infrastructure
# ---------------------------------------------------------------------------

$script:PassCount = 0
$script:FailCount = 0
$script:Results   = [System.Collections.Generic.List[string]]::new()

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

# Asserts that a script block throws (any exception)
function Assert-Throws {
  param([scriptblock]$Body, [string]$Because = "")
  try {
    & $Body
    throw "Expected an exception but none was thrown. $Because"
  }
  catch {
    if ($_.Exception.Message -like "Expected an exception*") { throw }
    # otherwise: expected — swallow
  }
}

# Asserts that a script block exits with a non-zero exit code when run in a
# child process.  Used for tests that call the full verify-manifest.ps1.
function Assert-ExitNonZero {
  param([string[]]$ScriptArgs, [string]$OutputPattern = "")

  $psi = [System.Diagnostics.ProcessStartInfo]::new()
  $psi.FileName  = (Get-Command pwsh -ErrorAction SilentlyContinue)?.Source
  if (-not $psi.FileName) { $psi.FileName = "pwsh" }
  $psi.Arguments = "-NonInteractive -File `"$VerifyScript`" " + ($ScriptArgs -join " ")
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError  = $true
  $psi.UseShellExecute = $false

  $proc = [System.Diagnostics.Process]::Start($psi)
  $stdout = $proc.StandardOutput.ReadToEnd()
  $proc.WaitForExit()

  if ($proc.ExitCode -eq 0) {
    throw "Expected non-zero exit code but got 0.`nOutput: $stdout"
  }

  if ($OutputPattern -and $stdout -notmatch $OutputPattern) {
    throw "Exit code was non-zero but output did not match pattern '$OutputPattern'.`nOutput: $stdout"
  }
}

# Runs verify-manifest.ps1 in-process via dot-sourcing in a child scope with
# a mock Invoke-StellarRead.  Returns captured output lines.
function Invoke-VerifyWithMock {
  param(
    [hashtable]$MockTable,       # key = "$ContractId|$Function[|$ArgString]" → return value
    [string]$ManifestPath = $TestnetManifest,
    [switch]$ExpectFailure,
    [string]$FailPattern = ""
  )

  # Build a temporary wrapper script that defines the mock, overrides
  # Invoke-StellarRead, then dot-sources verify-manifest.ps1.
  $tempScript = [System.IO.Path]::GetTempFileName() + ".ps1"

  # Serialise the mock table as a PowerShell literal hashtable
  $htLines = @('$MockTable = @{')
  foreach ($kv in $MockTable.GetEnumerator()) {
    $escapedKey = $kv.Key   -replace "'", "''"
    $escapedVal = $kv.Value -replace "'", "''"
    $htLines += "  '$escapedKey' = '$escapedVal'"
  }
  $htLines += '}'

  $wrapperLines = $htLines + @(
    'function Invoke-StellarRead {'
    '  param([string]$ContractId,[string]$Function,[string[]]$Args=@(),[string]$Network,[string]$CliPath,[int]$TimeoutSeconds,[int]$MaxRetries)'
    '  $key = "$ContractId|$Function"'
    '  if ($Args.Count -gt 0) { $argStr = ($Args | Where-Object { $_ -notmatch "^--" }) -join ","; if ($argStr) { $key += "|$argStr" } }'
    '  if (-not $MockTable.ContainsKey($key)) { throw "Unexpected mock call: $key" }'
    '  $val = $MockTable[$key]'
    '  if ($val -eq "__TIMEOUT__") { throw [System.TimeoutException]"mock timeout" }'
    '  if ($val -like "__ERROR__:*") { throw ($val -replace "^__ERROR__:","") }'
    '  return $val'
    '}'
    ". '$($VerifyScript -replace "'","''")' -Manifest '$($ManifestPath -replace "'","''")' -Live -Network stellar-testnet"
  )

  $wrapperLines | Set-Content $tempScript -Encoding UTF8

  try {
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName  = (Get-Command pwsh -ErrorAction SilentlyContinue)?.Source
    if (-not $psi.FileName) { $psi.FileName = "pwsh" }
    $psi.Arguments = "-NonInteractive -File `"$tempScript`""
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.UseShellExecute = $false

    $proc = [System.Diagnostics.Process]::Start($psi)
    $stdout = $proc.StandardOutput.ReadToEnd()
    $stderr = $proc.StandardError.ReadToEnd()
    $proc.WaitForExit()

    $combined = ($stdout + "`n" + $stderr).Trim()

    if ($ExpectFailure) {
      if ($proc.ExitCode -eq 0) {
        throw "Expected non-zero exit but got 0.`nOutput: $combined"
      }
      if ($FailPattern -and $combined -notmatch $FailPattern) {
        throw "Expected failure with pattern '$FailPattern' but output was:`n$combined"
      }
    }
    else {
      if ($proc.ExitCode -ne 0) {
        throw "Expected success (exit 0) but got $($proc.ExitCode).`nOutput: $combined"
      }
    }

    return $combined
  }
  finally {
    Remove-Item $tempScript -Force -ErrorAction SilentlyContinue
  }
}

# Default "happy path" mock responses (all contracts healthy, admin matches)
function Get-HappyMock {
  return @{
    "$PC_ID|get_admin"            = $ADMIN
    "$PC_ID|is_paused"            = "false"
    "$PC_ID|get_config_version"   = "1"
    "$PC_ID|is_schema_approved|1" = "true"
    "$IR_ID|get_admin"            = $ADMIN
    "$IR_ID|get_issuer_status|$ISSUER_ADDR" = "Active"
    "$PR_ID|get_admin"            = $ADMIN
    "$PR_ID|get_issuer_registry"  = $IR_ID
    "$PR_ID|get_protocol_config"  = $PC_ID
  }
}

# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

Write-Host ""
Write-Host "=== verify-manifest tests ===" -ForegroundColor Cyan
Write-Host ""

# 1. Offline mode passes with valid testnet manifest
Invoke-Test "offline mode passes with valid testnet manifest" {
  $psi = [System.Diagnostics.ProcessStartInfo]::new()
  $psi.FileName  = (Get-Command pwsh -ErrorAction SilentlyContinue)?.Source
  if (-not $psi.FileName) { $psi.FileName = "pwsh" }
  $psi.Arguments = "-NonInteractive -File `"$VerifyScript`" -Manifest `"$TestnetManifest`""
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError  = $true
  $psi.UseShellExecute = $false

  $proc = [System.Diagnostics.Process]::Start($psi)
  $out  = $proc.StandardOutput.ReadToEnd()
  $proc.WaitForExit()

  if ($proc.ExitCode -ne 0) {
    throw "Offline mode exited $($proc.ExitCode).`n$out"
  }
  if ($out -notmatch "valid") {
    throw "Expected 'valid' in output, got: $out"
  }
}

# 2. Live mode: all checks pass (happy path)
Invoke-Test "live mode admin match passes" {
  $output = Invoke-VerifyWithMock -MockTable (Get-HappyMock)
  if ($output -notmatch "All live on-chain checks passed") {
    throw "Expected success message, got: $output"
  }
}

# 3. Live mode: protocolConfig admin mismatch → exits non-zero with MISMATCH
Invoke-Test "live mode admin mismatch exits non-zero with MISMATCH message" {
  $mock = Get-HappyMock
  $mock["$PC_ID|get_admin"] = "GABCWRONGADDRESSXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"

  $output = Invoke-VerifyWithMock -MockTable $mock -ExpectFailure -FailPattern "MISMATCH"
  if ($output -notmatch "protocolConfig admin") {
    throw "Expected MISMATCH label 'protocolConfig admin', got: $output"
  }
}

# 4. Live mode: malformed CLI output from get_config_version → throws clear error
Invoke-Test "live mode malformed CLI output throws clear error" {
  $mock = Get-HappyMock
  $mock["$PC_ID|get_config_version"] = "not-a-number"

  $output = Invoke-VerifyWithMock -MockTable $mock -ExpectFailure -FailPattern "Malformed output from get_config_version"
}

# 5. Live mode: CLI timeout → retries then fails
#    We simulate a hard CLI error (rather than a real process timeout) because
#    there is no real stellar binary in CI.  The mock throws __TIMEOUT__ which
#    Invoke-StellarRead catches and retries; after MaxRetries it throws.
Invoke-Test "live mode CLI timeout retries then fails" {
  # All calls succeed except get_admin on protocolConfig which always times out
  $mock = Get-HappyMock
  $mock["$PC_ID|get_admin"] = "__TIMEOUT__"

  # The wrapper script passes -MaxRetries 1 so we don't wait forever.
  # We reuse Invoke-VerifyWithMock but need to customise MaxRetries — easier
  # to inline a small variant here.
  $tempScript = [System.IO.Path]::GetTempFileName() + ".ps1"
  $htLines = @('$MockTable = @{')
  foreach ($kv in $mock.GetEnumerator()) {
    $escapedKey = $kv.Key   -replace "'", "''"
    $escapedVal = $kv.Value -replace "'", "''"
    $htLines += "  '$escapedKey' = '$escapedVal'"
  }
  $htLines += '}'

  $wrapperLines = $htLines + @(
    'function Invoke-StellarRead {'
    '  param([string]$ContractId,[string]$Function,[string[]]$Args=@(),[string]$Network,[string]$CliPath,[int]$TimeoutSeconds,[int]$MaxRetries)'
    '  $key = "$ContractId|$Function"'
    '  if ($Args.Count -gt 0) { $argStr = ($Args | Where-Object { $_ -notmatch "^--" }) -join ","; if ($argStr) { $key += "|$argStr" } }'
    '  if (-not $MockTable.ContainsKey($key)) { throw "Unexpected mock call: $key" }'
    '  $val = $MockTable[$key]'
    '  if ($val -eq "__TIMEOUT__") { throw [System.TimeoutException]"mock timeout" }'
    '  if ($val -like "__ERROR__:*") { throw ($val -replace "^__ERROR__:","") }'
    '  return $val'
    '}'
    ". '$($VerifyScript -replace "'","''")' -Manifest '$($TestnetManifest -replace "'","''")' -Live -Network stellar-testnet -MaxRetries 1"
  )
  $wrapperLines | Set-Content $tempScript -Encoding UTF8

  try {
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName  = (Get-Command pwsh -ErrorAction SilentlyContinue)?.Source
    if (-not $psi.FileName) { $psi.FileName = "pwsh" }
    $psi.Arguments = "-NonInteractive -File `"$tempScript`""
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.UseShellExecute = $false

    $proc = [System.Diagnostics.Process]::Start($psi)
    $combined = ($proc.StandardOutput.ReadToEnd() + "`n" + $proc.StandardError.ReadToEnd()).Trim()
    $proc.WaitForExit()

    if ($proc.ExitCode -eq 0) {
      throw "Expected non-zero exit on persistent timeout, got 0.`n$combined"
    }
    if ($combined -notmatch "(?i)(timed out|timeout|mock timeout)") {
      throw "Expected timeout message, got: $combined"
    }
  }
  finally {
    Remove-Item $tempScript -Force -ErrorAction SilentlyContinue
  }
}

# 6. Live mode: transient RPC failure → retries and succeeds on second attempt
#    We model this by making the mock return a transient error string on the
#    first call and the real value on the second, via a counter file.
Invoke-Test "live mode transient RPC failure retries and succeeds" {
  $counterFile = [System.IO.Path]::GetTempFileName()
  Set-Content $counterFile "0"

  $tempScript = [System.IO.Path]::GetTempFileName() + ".ps1"

  $happyMock = Get-HappyMock
  $htLines = @('$MockTable = @{')
  foreach ($kv in $happyMock.GetEnumerator()) {
    $escapedKey = $kv.Key   -replace "'", "''"
    $escapedVal = $kv.Value -replace "'", "''"
    $htLines += "  '$escapedKey' = '$escapedVal'"
  }
  $htLines += '}'

  $counterEsc = $counterFile -replace "'", "''"

  $wrapperLines = $htLines + @(
    '$CallCount = [int](Get-Content "' + $counterEsc + '")'
    'function Invoke-StellarRead {'
    '  param([string]$ContractId,[string]$Function,[string[]]$Args=@(),[string]$Network,[string]$CliPath,[int]$TimeoutSeconds,[int]$MaxRetries)'
    '  # First call to get_admin on protocolConfig simulates a transient 503 error'
    '  if ($ContractId -eq "' + $PC_ID + '" -and $Function -eq "get_admin") {'
    '    $script:CallCount++'
    '    Set-Content "' + $counterEsc + '" $script:CallCount'
    '    if ($script:CallCount -eq 1) { throw "connection reset: 503 temporarily unavailable" }'
    '  }'
    '  $key = "$ContractId|$Function"'
    '  if ($Args.Count -gt 0) { $argStr = ($Args | Where-Object { $_ -notmatch "^--" }) -join ","; if ($argStr) { $key += "|$argStr" } }'
    '  if (-not $MockTable.ContainsKey($key)) { throw "Unexpected mock call: $key" }'
    '  return $MockTable[$key]'
    '}'
    ". '$($VerifyScript -replace "'","''")' -Manifest '$($TestnetManifest -replace "'","''")' -Live -Network stellar-testnet -MaxRetries 3"
  )
  $wrapperLines | Set-Content $tempScript -Encoding UTF8

  try {
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName  = (Get-Command pwsh -ErrorAction SilentlyContinue)?.Source
    if (-not $psi.FileName) { $psi.FileName = "pwsh" }
    $psi.Arguments = "-NonInteractive -File `"$tempScript`""
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.UseShellExecute = $false

    $proc = [System.Diagnostics.Process]::Start($psi)
    $combined = ($proc.StandardOutput.ReadToEnd() + "`n" + $proc.StandardError.ReadToEnd()).Trim()
    $proc.WaitForExit()

    if ($proc.ExitCode -ne 0) {
      throw "Expected success after retry but got exit $($proc.ExitCode).`n$combined"
    }
    if ($combined -notmatch "All live on-chain checks passed") {
      throw "Expected success message, got: $combined"
    }

    $finalCount = [int](Get-Content $counterFile)
    if ($finalCount -lt 2) {
      throw "Expected at least 2 calls (retry) but counter = $finalCount"
    }
  }
  finally {
    Remove-Item $tempScript   -Force -ErrorAction SilentlyContinue
    Remove-Item $counterFile  -Force -ErrorAction SilentlyContinue
  }
}

# 7. Live mode: is_paused returns true → fails
Invoke-Test "live mode is_paused true fails" {
  $mock = Get-HappyMock
  $mock["$PC_ID|is_paused"] = "true"

  $output = Invoke-VerifyWithMock -MockTable $mock -ExpectFailure -FailPattern "(?i)(paused|FAIL)"
}

# 8. Live mode: schema version not approved → fails
Invoke-Test "live mode schema version not approved fails" {
  $mock = Get-HappyMock
  $mock["$PC_ID|is_schema_approved|1"] = "false"

  $output = Invoke-VerifyWithMock -MockTable $mock -ExpectFailure -FailPattern "(?i)(schema|approved|FAIL)"
}

# 9. Live mode: issuerRegistry admin mismatch → exits non-zero with MISMATCH
Invoke-Test "live mode issuerRegistry admin mismatch fails" {
  $mock = Get-HappyMock
  $mock["$IR_ID|get_admin"] = "GWRONGAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB"

  $output = Invoke-VerifyWithMock -MockTable $mock -ExpectFailure -FailPattern "MISMATCH"
  if ($output -notmatch "issuerRegistry admin") {
    throw "Expected MISMATCH label 'issuerRegistry admin', got: $output"
  }
}

# 10. Live mode: proofRegistry cross-contract reference mismatch → fails
Invoke-Test "live mode proofRegistry issuerRegistry reference mismatch fails" {
  $mock = Get-HappyMock
  $mock["$PR_ID|get_issuer_registry"] = "CWRONGCONTRACTIDXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"

  $output = Invoke-VerifyWithMock -MockTable $mock -ExpectFailure -FailPattern "MISMATCH"
  if ($output -notmatch "issuerRegistry reference") {
    throw "Expected MISMATCH label for issuerRegistry reference, got: $output"
  }
}

# 11. Live mode: initialIssuer status NotFound → fails
Invoke-Test "live mode initialIssuer NotFound fails" {
  $mock = Get-HappyMock
  $mock["$IR_ID|get_issuer_status|$ISSUER_ADDR"] = "NotFound"

  $output = Invoke-VerifyWithMock -MockTable $mock -ExpectFailure -FailPattern "(?i)(NotFound|FAIL)"
}

# ---------------------------------------------------------------------------
# Release metadata validation (-Release)
# ---------------------------------------------------------------------------
# These cover the acceptance criterion that manifest verification rejects an
# artifact whose declared release metadata does not match what was deployed.
#
# Each case mutates a copy of the real release note rather than using a
# hand-written stub, so the fixture cannot drift away from the note the project
# actually ships.

$ReleaseNote = Join-Path $ScriptDir "..\docs\releases\v0.1.0.md"

# Writes a mutated copy of the release note to a temp file and returns its path.
function New-MutatedRelease {
  param([scriptblock]$Mutate)

  $text = Get-Content $ReleaseNote -Raw
  $mutated = & $Mutate $text
  $temp = [System.IO.Path]::GetTempFileName() + ".md"
  Set-Content -Path $temp -Value $mutated -Encoding UTF8
  return $temp
}

# Runs verify-manifest.ps1 in a child process and asserts it succeeded.
function Assert-ExitZero {
  param([string[]]$ScriptArgs, [string]$OutputPattern = "")

  $psi = [System.Diagnostics.ProcessStartInfo]::new()
  $psi.FileName = (Get-Command pwsh -ErrorAction SilentlyContinue)?.Source
  if (-not $psi.FileName) { $psi.FileName = "pwsh" }
  $psi.Arguments = "-NonInteractive -File `"$VerifyScript`" " + ($ScriptArgs -join " ")
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true
  $psi.UseShellExecute = $false

  $proc = [System.Diagnostics.Process]::Start($psi)
  $stdout = $proc.StandardOutput.ReadToEnd()
  $stderr = $proc.StandardError.ReadToEnd()
  $proc.WaitForExit()

  if ($proc.ExitCode -ne 0) {
    throw "Expected exit code 0 but got $($proc.ExitCode).`nOutput: $stdout`n$stderr"
  }

  if ($OutputPattern -and $stdout -notmatch $OutputPattern) {
    throw "Exited 0 but output did not match '$OutputPattern'.`nOutput: $stdout"
  }
}

# 12. A release note matching the manifest passes
Invoke-Test "release note matching the manifest passes" {
  Assert-ExitZero `
    -ScriptArgs @("-Manifest `"$TestnetManifest`"", "-Release `"$ReleaseNote`"") `
    -OutputPattern "(?i)release metadata matches"
}

# 13. A WASM hash absent from the manifest is rejected
Invoke-Test "release note with an unknown WASM hash fails" {
  $temp = New-MutatedRelease {
    param($t)
    $t -replace "dd0a2d58bc634f09f94f92b09811714a25f36f4e0bd34c10dbac33238c84d594",
    "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
  }
  try {
    Assert-ExitNonZero `
      -ScriptArgs @("-Manifest `"$TestnetManifest`"", "-Release `"$temp`"") `
      -OutputPattern "(?i)(hash|manifest)"
  }
  finally { Remove-Item $temp -ErrorAction SilentlyContinue }
}

# 14. A contract ID that was not deployed is rejected
Invoke-Test "release note omitting a deployed contract ID fails" {
  $temp = New-MutatedRelease {
    param($t)
    $t -replace "CCMTAXBWN2ZGEDVKGHT6GQENZSTBSLQAGYGGKJWNMDSTVRT2QNMMNWRK",
    "CDXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
  }
  try {
    Assert-ExitNonZero `
      -ScriptArgs @("-Manifest `"$TestnetManifest`"", "-Release `"$temp`"") `
      -OutputPattern "(?i)(contract ID|proofRegistry)"
  }
  finally { Remove-Item $temp -ErrorAction SilentlyContinue }
}

# 15. A missing required section is rejected
Invoke-Test "release note missing a required section fails" {
  $temp = New-MutatedRelease { param($t) $t -replace "## Rollback", "## Notes" }
  try {
    Assert-ExitNonZero `
      -ScriptArgs @("-Manifest `"$TestnetManifest`"", "-Release `"$temp`"") `
      -OutputPattern "(?i)Rollback"
  }
  finally { Remove-Item $temp -ErrorAction SilentlyContinue }
}

# 16. A malformed commit field is rejected
Invoke-Test "release note with a short commit fails" {
  $temp = New-MutatedRelease {
    param($t)
    $t -replace "\*\*Commit:\*\* 09f9841c9af78e67c90f0eaab1039052b17b9a03", "**Commit:** 09f9841"
  }
  try {
    Assert-ExitNonZero `
      -ScriptArgs @("-Manifest `"$TestnetManifest`"", "-Release `"$temp`"") `
      -OutputPattern "(?i)Commit"
  }
  finally { Remove-Item $temp -ErrorAction SilentlyContinue }
}

# 17. A leaked Stellar secret seed is rejected
Invoke-Test "release note containing a secret seed fails" {
  # Synthetic, seed-shaped value. Never a real key.
  $temp = New-MutatedRelease {
    param($t)
    $t + "`n`nDeployer seed: SCFIRY65OQE7DFP5KLNS2PF2LVZMUZYJX4OZIEQ36N2IQANUB5XVYOJR`n"
  }
  try {
    Assert-ExitNonZero `
      -ScriptArgs @("-Manifest `"$TestnetManifest`"", "-Release `"$temp`"") `
      -OutputPattern "(?i)secret"
  }
  finally { Remove-Item $temp -ErrorAction SilentlyContinue }
}

# 18. A credential-shaped assignment is rejected
Invoke-Test "release note containing an API key assignment fails" {
  $temp = New-MutatedRelease {
    param($t)
    $t + "`n`napi_key: not-a-real-key-but-shaped-like-one`n"
  }
  try {
    Assert-ExitNonZero `
      -ScriptArgs @("-Manifest `"$TestnetManifest`"", "-Release `"$temp`"") `
      -OutputPattern "(?i)credential"
  }
  finally { Remove-Item $temp -ErrorAction SilentlyContinue }
}

# 19. A breaking release without a named approver is rejected
Invoke-Test "breaking release without an approver fails" {
  $temp = New-MutatedRelease {
    param($t)
    ($t -replace "\*\*Classification:\*\* additive", "**Classification:** breaking") `
      -replace "\*\*Breaking change approved by:\*\* Not required.*", "**Breaking change approved by:** not required"
  }
  try {
    Assert-ExitNonZero `
      -ScriptArgs @("-Manifest `"$TestnetManifest`"", "-Release `"$temp`"") `
      -OutputPattern "(?i)approving maintainer"
  }
  finally { Remove-Item $temp -ErrorAction SilentlyContinue }
}

# ---------------------------------------------------------------------------
# Manifest secret-hygiene scan (#64)
# ---------------------------------------------------------------------------
# The manifest itself — not just an accompanying release note — must never
# contain secret-shaped values. Mutates a copy of the real testnet manifest
# rather than a hand-written stub, for the same reason New-MutatedRelease
# does above.

function New-MutatedManifest {
  param([scriptblock]$Mutate)

  $text = Get-Content $TestnetManifest -Raw
  $mutated = & $Mutate $text
  $temp = [System.IO.Path]::GetTempFileName() + ".json"
  Set-Content -Path $temp -Value $mutated -Encoding UTF8
  return $temp
}

# 20. A manifest containing a Stellar secret seed is rejected
Invoke-Test "manifest containing a secret seed fails" {
  $temp = New-MutatedManifest {
    param($t)
    $t -replace '"source": "deployer"', '"source": "SBQNSHKUSA3EWL2WVMMGZJIA3WWZTPGSXVSCJHW6NM4PZ3O2GEOAV7DS"'
  }
  try {
    Assert-ExitNonZero `
      -ScriptArgs @("-Manifest `"$temp`"") `
      -OutputPattern "(?i)secret seed"
  }
  finally { Remove-Item $temp -ErrorAction SilentlyContinue }
}

# 21. A manifest with a credential-shaped field assignment is rejected
Invoke-Test "manifest containing an API key assignment fails" {
  $temp = New-MutatedManifest {
    param($t)
    $t -replace '"notes":', '"apiKey": "sk_live_abcdef1234567890",' + "`n  " + '"notes":'
  }
  try {
    Assert-ExitNonZero `
      -ScriptArgs @("-Manifest `"$temp`"") `
      -OutputPattern "(?i)secret-like content"
  }
  finally { Remove-Item $temp -ErrorAction SilentlyContinue }
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
