param(
    [string]$Fixture = "basic",
    [string]$CppRoot = "D:\githubs\namer\ds3\DS3_demo3",
    [string]$RustRoot = "D:\githubs\namer\tswn-core",
    [switch]$UseRef2CppBin,
    [switch]$FailOnDiff
)

$ErrorActionPreference = "Stop"

function Copy-TreeContent {
    param(
        [Parameter(Mandatory = $true)][string]$From,
        [Parameter(Mandatory = $true)][string]$To
    )
    if (-not (Test-Path $To)) {
        New-Item -ItemType Directory -Path $To | Out-Null
    }
    Copy-Item -Recurse -Force (Join-Path $From '*') $To
}

function Compare-Tree {
    param(
        [Parameter(Mandatory = $true)][string]$LeftRoot,
        [Parameter(Mandatory = $true)][string]$RightRoot,
        [Parameter(Mandatory = $true)][string[]]$Dirs
    )

    $diffs = New-Object System.Collections.Generic.List[string]
    foreach ($dir in $Dirs) {
        $leftDir = Join-Path $LeftRoot $dir
        $rightDir = Join-Path $RightRoot $dir

        if (-not (Test-Path $leftDir)) {
            $diffs.Add("MISSING_LEFT_DIR $dir")
            continue
        }
        if (-not (Test-Path $rightDir)) {
            $diffs.Add("MISSING_RIGHT_DIR $dir")
            continue
        }

        $leftFiles = Get-ChildItem -File -Recurse $leftDir
        foreach ($lf in $leftFiles) {
            $rel = $lf.FullName.Substring($leftDir.Length + 1)
            $rf = Join-Path $rightDir $rel
            if (-not (Test-Path $rf)) {
                $diffs.Add("MISSING_RIGHT_FILE $dir\$rel")
                continue
            }
            $lb = [IO.File]::ReadAllBytes($lf.FullName)
            $rb = [IO.File]::ReadAllBytes($rf)
            if (-not [Linq.Enumerable]::SequenceEqual($lb, $rb)) {
                $diffs.Add("DIFF $dir\$rel")
            }
        }

        $rightFiles = Get-ChildItem -File -Recurse $rightDir
        foreach ($rf in $rightFiles) {
            $rel = $rf.FullName.Substring($rightDir.Length + 1)
            $lf = Join-Path $leftDir $rel
            if (-not (Test-Path $lf)) {
                $diffs.Add("EXTRA_RIGHT_FILE $dir\$rel")
            }
        }
    }
    return $diffs
}

$fixtureRoot = Join-Path $RustRoot "crates\tswn_ds3\tests\fixtures\$Fixture"
if (-not (Test-Path $fixtureRoot)) {
    throw "fixture not found: $fixtureRoot"
}

$workRoot = Join-Path $RustRoot "target\compare-with-cpp\$Fixture"
$cppWork = Join-Path $workRoot "cpp"
$rustWork = Join-Path $workRoot "rust"

if (Test-Path $workRoot) {
    Remove-Item -Recurse -Force $workRoot
}
New-Item -ItemType Directory -Path $cppWork, $rustWork | Out-Null

if ($UseRef2CppBin) {
    $cppBinRoot = Join-Path $RustRoot "target\ds3-cpp-ref2"
    if (-not (Test-Path $cppBinRoot)) {
        throw "UseRef2CppBin specified but missing: $cppBinRoot"
    }
    Copy-TreeContent -From $cppBinRoot -To $cppWork
} else {
    Copy-TreeContent -From $CppRoot -To $cppWork
}

Copy-TreeContent -From $fixtureRoot -To $cppWork
Copy-TreeContent -From $fixtureRoot -To $rustWork

$cppExpected = Join-Path $cppWork "expected"
$rustExpected = Join-Path $rustWork "expected"
if (Test-Path $cppExpected) { Remove-Item -Recurse -Force $cppExpected }
if (Test-Path $rustExpected) { Remove-Item -Recurse -Force $rustExpected }

foreach ($dir in @("tmp", "file", "new", "out", "input")) {
    New-Item -ItemType Directory -Force -Path (Join-Path $cppWork $dir) | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $rustWork $dir) | Out-Null
}

$env:PATH = "D:\apps\llvm-mingw\bin;$env:PATH"

Write-Host "[1/4] running C++ all.exe"
Push-Location $cppWork
try {
    & .\all.exe | Out-Host
    $cppExit = $LASTEXITCODE
} finally {
    Pop-Location
}
if ($cppExit -ne 0) {
    throw "C++ run failed: all.exe exit code $cppExit"
}

Write-Host "[2/4] running Rust tswn_ds3 run"
Push-Location $RustRoot
try {
    cargo run -q -p tswn_ds3 -- run --root $rustWork | Out-Host
    $rustExit = $LASTEXITCODE
} finally {
    Pop-Location
}
if ($rustExit -ne 0) {
    throw "Rust run failed: tswn_ds3 exit code $rustExit"
}

Write-Host "[3/4] comparing trees"
$dirs = @("tmp", "file", "new", "out")
$diffs = Compare-Tree -LeftRoot $cppWork -RightRoot $rustWork -Dirs $dirs

$report = Join-Path $workRoot "diff-report.txt"
if ($diffs.Count -eq 0) {
    "NO_DIFF" | Set-Content -Encoding UTF8 $report
    Write-Host "[4/4] NO_DIFF"
} else {
    $diffs | Sort-Object | Set-Content -Encoding UTF8 $report
    Write-Host "[4/4] DIFF_FOUND, report: $report"
    Get-Content $report | ForEach-Object { Write-Host $_ }
}

Write-Host "cpp work : $cppWork"
Write-Host "rust work: $rustWork"
Write-Host "report   : $report"

if ($diffs.Count -gt 0 -and $FailOnDiff) {
    exit 2
}
