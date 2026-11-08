param(
    [Parameter(Mandatory = $true)][string]$Binary,
    [switch]$AllowMissingOpenGLContext
)
$ErrorActionPreference = "Stop"
$exe = [IO.Path]::GetFullPath($Binary)
$temporary = Join-Path ([IO.Path]::GetTempPath()) ("nswt-smoke-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temporary | Out-Null
$oldOffline = $env:NSWT_OFFLINE
$oldData = $env:NSWT_DATA_DIR
$process = $null
$guiSmokeStatus = "passed"
function Read-SharedText([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return "" }
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
    $reader = [IO.StreamReader]::new($stream)
    try { return $reader.ReadToEnd() } finally { $reader.Dispose() }
}
try {
    $env:NSWT_OFFLINE = "1"
    $env:NSWT_DATA_DIR = Join-Path $temporary "data"
    foreach ($case in @(
        @{ arguments = @("--help"); code = 0; contains = "template" },
        @{ arguments = @("--version"); code = 0; contains = "NekoSportsWorldTool" },
        @{ arguments = @("--definitely-invalid"); code = 1; contains = "未知命令" }
    )) {
        $stdout = Join-Path $temporary "stdout.txt"
        $stderr = Join-Path $temporary "stderr.txt"
        $cli = Start-Process -FilePath $exe -ArgumentList $case.arguments -Wait -PassThru -NoNewWindow -RedirectStandardOutput $stdout -RedirectStandardError $stderr
        if ($cli.ExitCode -ne $case.code) { throw "CLI exit mismatch for $($case.arguments): $($cli.ExitCode)" }
        $text = [IO.File]::ReadAllText($stdout) + [IO.File]::ReadAllText($stderr)
        if (-not $text.Contains($case.contains)) { throw "CLI output missing expected text: $($case.arguments)" }
    }
    for ($attempt = 1; $attempt -le 2; $attempt++) {
        $guiStdout = Join-Path $temporary "gui-$attempt.stdout.txt"
        $guiStderr = Join-Path $temporary "gui-$attempt.stderr.txt"
        $process = Start-Process -FilePath $exe -PassThru -WorkingDirectory $temporary -RedirectStandardOutput $guiStdout -RedirectStandardError $guiStderr
        $deadline = [DateTime]::UtcNow.AddSeconds(15)
        do {
            Start-Sleep -Milliseconds 250
            $process.Refresh()
            $diagnostic = Read-SharedText $guiStderr
            if ($diagnostic.Contains("NSWT_GUI_INITIALIZATION=error")) { throw "GUI initialization reported an error: $diagnostic" }
            if ($process.HasExited) {
                if ($AllowMissingOpenGLContext -and $process.ExitCode -eq 1 -and
                    $diagnostic.Contains("egui_glow: OpenGL: egui_glow requires opengl 2.0+.")) {
                    $guiSmokeStatus = "unavailable_missing_opengl_2_context"
                    $process.Dispose()
                    $process = $null
                    break
                }
                throw "GUI exited before ready (code $($process.ExitCode)): $diagnostic"
            }
            $ready = $diagnostic.Contains("NSWT_GUI_INITIALIZATION=ready")
        } until (($ready -and $process.MainWindowHandle -ne [IntPtr]::Zero) -or [DateTime]::UtcNow -ge $deadline)
        if ($guiSmokeStatus -ne "passed") { break }
        if (-not $ready -or $process.MainWindowHandle -eq [IntPtr]::Zero) { throw "GUI was not ready within 15 seconds: $diagnostic" }
        $process.Refresh()
        if (-not $process.Responding) { throw "GUI did not stay responsive" }
        if (-not $process.CloseMainWindow()) { throw "GUI has no responsive main window" }
        if (-not $process.WaitForExit(10000)) { throw "GUI did not exit after normal window close" }
        if ($process.ExitCode -ne 0) { throw "GUI exit code was $($process.ExitCode)" }
        $process.Dispose()
        $process = $null
    }
    if ($guiSmokeStatus -eq "passed") {
        Write-Output "NSWT_GUI_SMOKE=passed"
        Write-Host "Offline packaged-executable CLI/GUI smoke passed; visual/DPI acceptance remains pending."
    } else {
        Write-Output "NSWT_GUI_SMOKE=unavailable_missing_opengl_2_context"
        Write-Warning "The hosted runner has no OpenGL 2.0 context; GUI smoke could not run. Windows desktop acceptance remains pending."
    }
} finally {
    if ($process) {
        if (-not $process.HasExited) {
            Stop-Process -Id $process.Id -Force
            $process.WaitForExit(5000) | Out-Null
        }
        $process.Dispose()
    }
    $env:NSWT_OFFLINE = $oldOffline
    $env:NSWT_DATA_DIR = $oldData
    if (Test-Path -LiteralPath $temporary) {
        for ($attempt = 1; $attempt -le 10; $attempt++) {
            try {
                Remove-Item -LiteralPath $temporary -Recurse -Force -ErrorAction Stop
                break
            } catch {
                if ($attempt -eq 10) { throw }
                Start-Sleep -Milliseconds 250
            }
        }
    }
}
