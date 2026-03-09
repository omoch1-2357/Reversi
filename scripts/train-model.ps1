param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$TrainArgs
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

function Test-HasCliOption {
    param(
        [string[]]$OptionArgs,
        [string]$LongOption
    )

    foreach ($arg in $OptionArgs) {
        if ($arg -eq $LongOption -or $arg.StartsWith("$LongOption=")) {
            return $true
        }
    }

    return $false
}

function Add-DefaultCliOption {
    param(
        [System.Collections.Generic.List[string]]$OptionArgs,
        [string]$LongOption,
        [string]$Value
    )

    if (-not (Test-HasCliOption -OptionArgs $OptionArgs.ToArray() -LongOption $LongOption)) {
        $OptionArgs.Add($LongOption)
        $OptionArgs.Add($Value)
    }
}

function Invoke-Step {
    param(
        [string]$Executable,
        [string[]]$Arguments
    )

    Write-Host ">" $Executable ($Arguments -join " ")
    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $Executable $($Arguments -join ' ')"
    }
}

$pythonCommand = Get-Command python -ErrorAction SilentlyContinue
if (-not $pythonCommand) {
    throw "python executable was not found in PATH."
}

$pythonExe = $pythonCommand.Source
$maturinCommand = Get-Command maturin -ErrorAction SilentlyContinue
if (-not $maturinCommand) {
    throw "maturin executable was not found in PATH. Install python requirements first."
}

Invoke-Step $pythonExe @("-m", "pip", "install", "-r", "python/requirements.txt")
Invoke-Step $maturinCommand.Source @(
    "build",
    "--release",
    "--manifest-path",
    "python/rust_training_ext/Cargo.toml",
    "--out",
    "python/dist",
    "--interpreter",
    $pythonExe
)

$wheel = Get-ChildItem "python/dist/reversi_training_ext-*.whl" |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if (-not $wheel) {
    throw "No built wheel was found under python/dist."
}

Invoke-Step $pythonExe @("-m", "pip", "install", "--force-reinstall", $wheel.FullName)

$resolvedArgs = [System.Collections.Generic.List[string]]::new()
foreach ($arg in $TrainArgs) {
    $resolvedArgs.Add($arg)
}

Add-DefaultCliOption -OptionArgs $resolvedArgs -LongOption "--games" -Value "500000"
Add-DefaultCliOption -OptionArgs $resolvedArgs -LongOption "--threads" -Value "0"
Add-DefaultCliOption -OptionArgs $resolvedArgs -LongOption "--progress-interval" -Value "10000"
Add-DefaultCliOption -OptionArgs $resolvedArgs -LongOption "--output" -Value "rust/src/ai/weights.bin"

Invoke-Step $pythonExe (@("python/train.py") + $resolvedArgs.ToArray())
