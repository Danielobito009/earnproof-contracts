<#
.SYNOPSIS
Runs the bounded cargo-mutants profile for EarnProof and enforces the reviewed
mutation score.

.DESCRIPTION
The bounded profile is defined in .cargo/mutants.toml and targets only the
authorization and validation branches of the three on-chain contracts
(require_auth, admin equality, pause, issuer status, schema approval, duplicate
registration, expiry, and revocation). This script:

  * ensures a pinned, reproducible cargo-mutants version is installed,
  * runs the bounded profile,
  * reads mutants.out/outcomes.json to compute the mutation score, and
  * fails (exit 1) when the score falls below -MinimumScore.

Use -SelfTest to prove the gate actually catches the seeded mutation classes
from tests/mutation/seeds/ (a removed require_auth call and an inverted
validity check). The self-test applies each seed, runs the test suite, and
asserts that the suite FAILS - i.e. that the mutation is caught.

.EXAMPLE
.\scripts\mutation-test.ps1

.EXAMPLE
.\scripts\mutation-test.ps1 -SelfTest
#>
[CmdletBinding()]
param(
    [string]$CargoMutantsVersion = "27.1.0",
    [string]$Config = ".cargo/mutants.toml",
    [double]$MinimumScore = 100.0,
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"

function Get-CargoMutantsVersion {
    # Note: `cargo mutants -V` means `--unviable`, so use the long `--version`.
    $out = cargo mutants --version 2>$null
    if ($LASTEXITCODE -ne 0) { return $null }
    return ($out -join " ").Trim() -replace "cargo-mutants ", ""
}

function Install-CargoMutants {
    param([string]$Version)
    $installed = Get-CargoMutantsVersion

    if (-not $installed -or $installed -ne $Version) {
        Write-Host "Installing cargo-mutants $Version (reproducible, --locked)..."
        cargo install --locked cargo-mutants --version $Version
        if ($LASTEXITCODE -ne 0) { throw "cargo-mutants install failed" }
    }
    else {
        Write-Host "cargo-mutants $installed already installed."
    }
}

function Get-MutationScore {
    param([string]$OutcomesPath)
    if (-not (Test-Path $OutcomesPath)) {
        throw "outcomes.json not found at $OutcomesPath"
    }
    $outcomes = Get-Content $OutcomesPath -Raw | ConvertFrom-Json

    # cargo-mutants writes the summary counts as top-level fields of outcomes.json.
    $caught   = [int]($outcomes.caught)
    $missed   = [int]($outcomes.missed)
    $unviable = [int]($outcomes.unviable)
    $timeout  = [int]($outcomes.timeout)

    $total = $caught + $missed
    $score = if ($total -gt 0) { [math]::Round(100.0 * $caught / $total, 2) } else { 100.0 }

    Write-Host ""
    Write-Host "Mutation score: $score% ($caught caught, $missed missed, $unviable unviable, $timeout timeout)"

    return [pscustomobject]@{
        Score    = $score
        Caught   = $caught
        Missed   = $missed
        Unviable = $unviable
        Timeout  = $timeout
    }
}

function Run-MutationProfile {
    param([string]$ConfigPath)
    if (Test-Path $ConfigPath) {
        Write-Host "Running bounded mutation profile ($ConfigPath)..."
        cargo mutants --config $ConfigPath --no-times
        return $LASTEXITCODE
    }
    Write-Host "No config at $ConfigPath; running cargo mutants with repository defaults..."
    cargo mutants --no-times
    return $LASTEXITCODE
}

function Assert-SeedIsCaught {
    param([string]$PatchPath, [string]$Label)
    Write-Host ""
    Write-Host "== Seed: $Label =="

    git apply $PatchPath
    if ($LASTEXITCODE -ne 0) { throw "failed to apply seed $Label" }

    $seedExit = 0
    try {
        cargo test --workspace *> $null
        $seedExit = $LASTEXITCODE
    }
    finally {
        # Always restore the working tree, even if the test command throws.
        git checkout -- . 2>$null | Out-Null
    }

    if ($seedExit -eq 0) {
        throw "SELF-TEST FAILED: the test suite did NOT catch the seeded mutation '$Label'. The mutation gate is broken."
    }
    Write-Host "Seed caught: the test suite failed as expected for '$Label'."
}

# ---------------------------------------------------------------------------
# Self-test mode: prove the gate catches seeded authorization/validity mutations.
# ---------------------------------------------------------------------------
if ($SelfTest) {
    Write-Host "Running seeded-mutation self-test..."
    $seeds = @(Get-ChildItem "tests/mutation/seeds/*.patch" -ErrorAction SilentlyContinue)
    if ($seeds.Count -eq 0) {
        throw "No seed patches found under tests/mutation/seeds/"
    }
    foreach ($seed in $seeds) {
        Assert-SeedIsCaught -PatchPath $seed.FullName -Label $seed.Name
    }
    Write-Host ""
    Write-Host "All seeded mutations were caught. The gate is effective."
    exit 0
}

# ---------------------------------------------------------------------------
# Main: run the bounded profile and enforce the reviewed score.
# ---------------------------------------------------------------------------
Install-CargoMutants -Version $CargoMutantsVersion

$exitCode = Run-MutationProfile -ConfigPath $Config
if ($exitCode -notin @(0, 2, 3)) {
    # 4 = baseline failed, 70 = internal error, etc.
    throw "cargo mutants failed with exit code $exitCode"
}

$score = Get-MutationScore -OutcomesPath "mutants.out/outcomes.json"

if ($score.Missed -gt 0) {
    Write-Host "Missed mutants are listed in mutants.out/missed.txt and mutants.out/outcomes.json."
    Write-Host "Each missed mutant must be fixed with a test or explicitly justified (see docs/testing.md)."
}

if ($score.Score -lt $MinimumScore) {
    Write-Error "Mutation score $($score.Score)% is below the required $MinimumScore%."
    exit 1
}

Write-Host "Mutation score $($score.Score)% meets the required $MinimumScore% threshold."
exit 0
