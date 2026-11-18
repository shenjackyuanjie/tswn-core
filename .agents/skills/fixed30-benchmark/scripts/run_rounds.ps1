<#
.SYNOPSIS
  在 tswn-core 仓库跑 fixed30 多轮基准，逐轮落盘并记录前后 CPU 负载。

.DESCRIPTION
  调用 release 的 perf_runtime example，对 docs/perf/fixed_cases_30 按指定线程配置重复采样。
  每轮输出到 <OutDir>/t<threads>_r<round>.jsonl，元数据追加到 <OutDir>/runs.log，
  交给同目录的 summarize.mjs 汇总。任何一轮非零退出都会停止并报错。

.PARAMETER Rounds
  每种线程配置的轮数，默认 10。

.PARAMETER Threads
  线程配置列表，1 为单线程、0 为自动线程，默认 @(1, 0)。

.PARAMETER RunsPerCase
  每个 case 的场数，默认 13000（与历史留档口径一致）。

.PARAMETER OutDir
  输出目录，默认 target/fixed30_<yyyy-MM-dd_HHmm>。

.EXAMPLE
  pwsh .agents/skills/fixed30-benchmark/scripts/run_rounds.ps1 -Rounds 10
#>
[CmdletBinding()]
param(
  [int]$Rounds = 10,
  [int[]]$Threads = @(1, 0),
  [int]$RunsPerCase = 13000,
  [string]$InputDir = 'docs/perf/fixed_cases_30',
  [string]$Exe = 'target/release/examples/perf_runtime.exe',
  [string]$OutDir,
  [int]$IntervalSeconds = 3,
  [switch]$TimedRound
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $Exe)) {
  throw "找不到 $Exe。先执行：cargo build -p tswn_core --release --features no_debug --example perf_runtime"
}
if (-not (Test-Path $InputDir)) { throw "找不到输入目录 $InputDir" }
if (-not $OutDir) { $OutDir = "target/fixed30_$(Get-Date -Format 'yyyy-MM-dd_HHmm')" }
New-Item -ItemType Directory -Force $OutDir | Out-Null

$exePath = (Resolve-Path $Exe).Path
$inputPath = (Resolve-Path $InputDir).Path
$logPath = Join-Path $OutDir 'runs.log'
$startedAt = Get-Date
Write-Output ("采样开始 {0}｜输出 {1}｜轮数 {2}｜线程 {3}｜每 case {4} 场" -f $startedAt.ToString('HH:mm:ss'), $OutDir, $Rounds, ($Threads -join ','), $RunsPerCase)

function Invoke-Round {
  param([int]$ThreadCount, [int]$Round, [string]$Suffix = '')

  Start-Sleep -Seconds $IntervalSeconds
  $loadBefore = [math]::Round((Get-CimInstance Win32_Processor | Measure-Object LoadPercentage -Average).Average, 0)
  $t0 = Get-Date
  $target = Join-Path $OutDir ("t{0}_r{1}{2}.jsonl" -f $ThreadCount, $Round, $Suffix)
  $arguments = @('-i', $inputPath, '-n', "$RunsPerCase", '--threads', "$ThreadCount")
  if ($Suffix -eq '_timed') { $arguments += '--timed' }
  $sw = Measure-Command {
    $proc = Start-Process -FilePath $exePath -ArgumentList $arguments -RedirectStandardOutput $target -NoNewWindow -PassThru -Wait
    $script:exitCode = $proc.ExitCode
  }
  $t1 = Get-Date
  $loadAfter = [math]::Round((Get-CimInstance Win32_Processor | Measure-Object LoadPercentage -Average).Average, 0)
  $line = ("threads={0} round={1} wall_s={2} start={3} end={4} load_before={5} load_after={6} exit={7}" -f $ThreadCount, $Round, [math]::Round($sw.TotalSeconds, 3), $t0.ToString('HH:mm:ss'), $t1.ToString('HH:mm:ss'), $loadBefore, $loadAfter, $script:exitCode)
  Write-Output $line
  Add-Content -Path $logPath -Value $line
  if ($script:exitCode -ne 0) { throw ("第 {0} 轮（threads={1}）退出码 {2}，停止采样" -f $Round, $ThreadCount, $script:exitCode) }
}

$script:exitCode = 0
foreach ($threadCount in $Threads) {
  for ($round = 1; $round -le $Rounds; $round += 1) {
    Invoke-Round -ThreadCount $threadCount -Round $round
  }
}

if ($TimedRound) {
  Invoke-Round -ThreadCount 1 -Round 0 -Suffix '_timed'
  Write-Output 'timed 轮只用于 init/fight 分解，不参与多轮统计。'
}

$finishedAt = Get-Date
Write-Output ("采样结束 {0}｜总耗时 {1:N1} 秒｜原始数据 {2}" -f $finishedAt.ToString('HH:mm:ss'), ($finishedAt - $startedAt).TotalSeconds, $OutDir)
Write-Output ("汇总：node .agents/skills/fixed30-benchmark/scripts/summarize.mjs --dir {0}" -f $OutDir)
