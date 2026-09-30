param(
    [string]$CppRoot = "",
    [string]$Fixture = "ds4_preview",
    [switch]$FailOnDiff,
    [ValidateRange(1, 10)]
    [int]$Rounds = 2
)

$ErrorActionPreference = "Stop"
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..\..")).Path
if (-not $CppRoot) {
    $CppRoot = Join-Path $repoRoot "..\Data_Structure4.0"
}
$CppRoot = (Resolve-Path -LiteralPath $CppRoot).Path
$fixtureRoot = (Resolve-Path -LiteralPath (Join-Path $repoRoot "crates\tswn_ds4\tests\fixtures\$Fixture")).Path
$stamp = Get-Date -Format "yyyyMMdd-HHmmss-fff"
$workRoot = Join-Path $repoRoot "target\compare-ds4\$stamp"
$cppWork = Join-Path $workRoot "cpp"
$rustWork = Join-Path $workRoot "rust"

# 每次创建独立目录，避免覆盖已有的增量状态和结果。
foreach ($work in @($cppWork, $rustWork)) {
    New-Item -ItemType Directory -Path (Join-Path $work "input") -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $fixtureRoot "config.json") -Destination $work
    Copy-Item -Path (Join-Path $fixtureRoot "input\*") -Destination (Join-Path $work "input")
}
New-Item -ItemType Directory -Path (Join-Path $cppWork "abcp5") -Force | Out-Null
Get-ChildItem -LiteralPath $CppRoot -File -Filter "*.exe" | Copy-Item -Destination $cppWork
Get-ChildItem -LiteralPath (Join-Path $CppRoot "abcp5") -File | Copy-Item -Destination (Join-Path $cppWork "abcp5")
Copy-Item -LiteralPath (Join-Path $CppRoot "score_now.txt") -Destination $cppWork

# 每轮都核对结果，第二轮验证没有新输入时历史结果和预测缓存保持一致。
for ($round = 1; $round -le $Rounds; $round++) {
    Write-Host "第 $round/$Rounds 轮"
    Write-Host "[1/3] 运行 DS4 C++ all_new.exe"
    Push-Location $cppWork
    try {
        & .\all_new.exe | Out-Host
        if ($LASTEXITCODE -ne 0) { throw "all_new.exe 退出码: $LASTEXITCODE" }
    } finally {
        Pop-Location
    }

    Write-Host "[2/3] 运行 Rust tswn_ds4"
    $priorAbcpDir = $env:TSWN_DS4_ABCP_DIR
    $env:TSWN_DS4_ABCP_DIR = Join-Path $cppWork "abcp5"
    Push-Location $repoRoot
    try {
        cargo run -q -p tswn_ds4 -- run --root $rustWork | Out-Host
        if ($LASTEXITCODE -ne 0) { throw "tswn_ds4 退出码: $LASTEXITCODE" }
    } finally {
        Pop-Location
        $env:TSWN_DS4_ABCP_DIR = $priorAbcpDir
    }

    Write-Host "[3/3] 比较文本记录集合"
    $diffs = [Collections.Generic.List[string]]::new()
    foreach ($dir in @("tmp", "file", "new", "out", "3ren", "abcp5")) {
        $cppDir = Join-Path $cppWork $dir
        foreach ($file in Get-ChildItem -LiteralPath $cppDir -File -Recurse -Filter "*.txt" -ErrorAction SilentlyContinue) {
            if ($dir -eq "abcp5" -and $file.Name -notlike "result*.txt") { continue }
            $relative = $file.FullName.Substring($cppWork.Length + 1)
            $rustFile = Join-Path $rustWork $relative
            if (-not (Test-Path -LiteralPath $rustFile)) {
                $diffs.Add("MISSING $relative")
                continue
            }
            $cppLines = [string[]]@(Get-Content -LiteralPath $file.FullName)
            $rustLines = [string[]]@(Get-Content -LiteralPath $rustFile)
            [Array]::Sort($cppLines, [StringComparer]::Ordinal)
            [Array]::Sort($rustLines, [StringComparer]::Ordinal)
            if (-not [Linq.Enumerable]::SequenceEqual($cppLines, $rustLines)) {
                $diffs.Add("DIFF $relative C++=$($cppLines.Count) Rust=$($rustLines.Count)")
            }
        }
    }
    $report = Join-Path $workRoot "diff-report-$round.txt"
    if ($diffs.Count -eq 0) {
        "NO_DIFF" | Set-Content -LiteralPath $report -Encoding utf8
        Write-Host "NO_DIFF: $report"
    } else {
        $diffs | Set-Content -LiteralPath $report -Encoding utf8
        Get-Content -LiteralPath $report | Out-Host
        if ($FailOnDiff) { exit 2 }
    }

}
