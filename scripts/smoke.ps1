# Release smoke test: launches paint.exe, measures time until the main window
# appears and the peak working set, and fails if a quality target is missed.
param(
    [string]$Exe = "target/release/paint.exe",
    [double]$MaxStartSeconds = 3.0,
    [double]$MaxMemoryMB = 500,
    [int]$RunSeconds = 5
)
$ErrorActionPreference = "Stop"

$sw = [Diagnostics.Stopwatch]::StartNew()
$p = Start-Process -FilePath $Exe -PassThru
while ($sw.Elapsed.TotalSeconds -lt 30) {
    $p.Refresh()
    if ($p.HasExited) { throw "paint.exe exited during startup (exit code $($p.ExitCode))" }
    if ($p.MainWindowHandle -ne [IntPtr]::Zero) { break }
    Start-Sleep -Milliseconds 20
}
$startup = [math]::Round($sw.Elapsed.TotalSeconds, 2)
if ($p.MainWindowHandle -eq [IntPtr]::Zero) {
    Stop-Process -Id $p.Id -Force
    throw "main window did not appear within 30 s"
}

Start-Sleep -Seconds $RunSeconds
$p.Refresh()
if ($p.HasExited) { throw "paint.exe crashed after startup (exit code $($p.ExitCode))" }
$memory = [math]::Round($p.PeakWorkingSet64 / 1MB, 1)
Stop-Process -Id $p.Id -Force

$result = "| 項目 | 結果 | 目標 |`n|---|---|---|`n| 起動時間 (ウィンドウ表示まで) | $startup s | ≤ $MaxStartSeconds s |`n| ピークメモリ (Working Set) | $memory MB | ≤ $MaxMemoryMB MB |"
Write-Output $result
if ($env:GITHUB_STEP_SUMMARY) { $result | Out-File -FilePath $env:GITHUB_STEP_SUMMARY -Append -Encoding utf8 }

if ($startup -gt $MaxStartSeconds) { throw "startup $startup s exceeds $MaxStartSeconds s" }
if ($memory -gt $MaxMemoryMB) { throw "peak memory $memory MB exceeds $MaxMemoryMB MB" }
