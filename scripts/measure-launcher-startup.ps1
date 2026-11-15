# Đo thời gian khởi động của F-Launcher (tasks.md 15.2: dưới 3 giây).
#
# Chạy launcher nhiều lần với FECO_LAUNCHER_STARTUP_PROBE: launcher ghi số mili giây từ lúc tiến
# trình bắt đầu tới khung hình đầu tiên rồi tự thoát. Script in từng lần, trung vị và lần lâu nhất.
#
#   powershell -ExecutionPolicy Bypass -File scripts\measure-launcher-startup.ps1 -Exe <feco-launcher.exe> [-Runs 5]
#
# Cần các biến môi trường launcher thường dùng (FECO_LAUNCHER_TUF, FECO_LAUNCHER_CLIENT_ID...),
# ví dụ đặt sẵn như .dev\run-launcher.ps1.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [int]$Runs = 5,
    [int]$LimitMs = 3000
)

$results = @()
for ($i = 1; $i -le $Runs; $i++) {
    $out = Join-Path ([System.IO.Path]::GetTempPath()) ("feco-startup-" + [guid]::NewGuid() + ".txt")
    $env:FECO_LAUNCHER_STARTUP_PROBE = $out
    $wall = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Exe -PassThru
    if (-not $p.WaitForExit(60000)) {
        Stop-Process -Id $p.Id -Force
        throw "Lần $i : launcher không thoát sau 60 giây"
    }
    $wall.Stop()
    if (-not (Test-Path $out)) { throw "Lần $i : launcher không ghi kết quả đo" }
    $ms = [int](Get-Content $out -Raw).Trim()
    Remove-Item $out -Force
    "Lần {0}: khung hình đầu sau {1} ms (cả lúc thoát: {2} ms)" -f $i, $ms, $wall.ElapsedMilliseconds
    $results += $ms
}
Remove-Item Env:\FECO_LAUNCHER_STARTUP_PROBE
$sorted = $results | Sort-Object
$median = $sorted[[int][math]::Floor(($sorted.Count - 1) / 2)]
$max = $sorted[-1]
"Trung vị: $median ms; lâu nhất: $max ms; ngưỡng: $LimitMs ms"
if ($max -ge $LimitMs) { exit 1 }
