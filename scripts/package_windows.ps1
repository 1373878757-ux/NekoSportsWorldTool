param(
    [string]$Binary = "target/x86_64-pc-windows-msvc/release/nekosportsworldtool.exe",
    [string]$OutputDirectory = "dist"
)

$ErrorActionPreference = "Stop"
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Set-Location $repo
$binaryPath = [IO.Path]::GetFullPath($Binary)
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) { throw "EXE missing: $Binary" }
$bytes = [IO.File]::ReadAllBytes($binaryPath)
if ($bytes.Length -lt 256 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) { throw "Not a PE executable" }
$pe = [BitConverter]::ToInt32($bytes, 0x3c)
if ($pe -lt 0 -or $pe + 26 -gt $bytes.Length -or [BitConverter]::ToUInt32($bytes, $pe) -ne 0x4550) { throw "Invalid PE header" }
if ([BitConverter]::ToUInt16($bytes, $pe + 4) -ne 0x8664 -or [BitConverter]::ToUInt16($bytes, $pe + 24) -ne 0x20b) { throw "Expected Windows x86_64 PE32+" }

$identity = [IO.File]::ReadAllText((Join-Path $repo "src/presentation.rs"))
$expectedName = [regex]::Match($identity, 'APP_NAME: &str = "([^"]+)"').Groups[1].Value
$expectedCredit = [regex]::Match($identity, 'SIGNATURE: &str = "([^"]+)"').Groups[1].Value
$info = (Get-Item -LiteralPath $binaryPath).VersionInfo
if ($info.ProductName -cne $expectedName -or $info.Comments -cne $expectedCredit) { throw "PE identity/credit mismatch" }
if ($info.LegalCopyright -notmatch "YanamiNeko") { throw "Upstream copyright missing" }
$version = [regex]::Match([IO.File]::ReadAllText((Join-Path $repo "Cargo.toml")), '(?m)^version = "([^"]+)"').Groups[1].Value
if ($info.ProductVersion -ne $version) { throw "PE version mismatch: $($info.ProductVersion) / $version" }

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio/Installer/vswhere.exe"
$vs = (& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1)
if (-not $vs) { throw "MSVC installation not found for dependency inspection" }
$tools = (Get-Content -LiteralPath (Join-Path $vs "VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt") -Raw).Trim()
$dumpbin = Join-Path $vs "VC/Tools/MSVC/$tools/bin/Hostx64/x64/dumpbin.exe"
$dependencyOutput = & $dumpbin /DEPENDENTS $binaryPath
if ($LASTEXITCODE -ne 0) { throw "dumpbin /DEPENDENTS failed" }
$dlls = @($dependencyOutput | ForEach-Object { if ($_ -match '^\s+([A-Za-z0-9_.-]+\.dll)\s*$') { $matches[1] } })
if ($dlls.Count -eq 0) { throw "No Windows system imports found" }
if ($dlls | Where-Object { $_ -match '^(VCRUNTIME|MSVCP|CONCRT|MSVCR)\d' }) { throw "Unexpected dynamically linked VC runtime" }
foreach ($dll in $dlls) {
    # API-set contracts are resolved by the Windows loader and need no same-named disk file.
    if ($dll -match '^(api|ext)-ms-win-[a-z0-9-]+-l\d+-\d+-\d+\.dll$') { continue }
    if (-not (Test-Path -LiteralPath (Join-Path $env:WINDIR "System32/$dll"))) { throw "Non-system DLL requires explicit packaging: $dll" }
}
$signatureStatus = (Get-AuthenticodeSignature -LiteralPath $binaryPath).Status.ToString()

$output = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $output | Out-Null
$archive = Join-Path $output "NekoSportsWorldTool-J-claude-Windows-x64.zip"
if (Test-Path -LiteralPath $archive) { throw "Output already exists; choose a clean output directory: $archive" }
$temporary = Join-Path ([IO.Path]::GetTempPath()) ("nswt-package-" + [guid]::NewGuid().ToString("N"))
$package = Join-Path $temporary "NekoSportsWorldTool-J-claude"
New-Item -ItemType Directory -Path $package -Force | Out-Null
try {
    Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $package "NekoSportsWorldTool.exe")
    Copy-Item -LiteralPath (Join-Path $repo "LICENSE") -Destination $package
    Copy-Item -LiteralPath (Join-Path $repo "README-Windows.md") -Destination $package
    Copy-Item -LiteralPath (Join-Path $repo "README-Portable.md") -Destination (Join-Path $package "使用说明-简体中文.md")
    & python (Join-Path $PSScriptRoot "collect_licenses.py") --target x86_64-pc-windows-msvc --output $package
    if ($LASTEXITCODE -ne 0) { throw "Dependency license collection failed" }
    $commit = (& git rev-parse HEAD).Trim()
    $buildInfo = @(
        "Product: $expectedName", "Version: $version", "Commit: $commit",
        "Target: x86_64-pc-windows-msvc", "Credit: $expectedCredit",
        "Signature status: $signatureStatus", "C runtime: static; Windows system DLLs remain required",
        "System imports: $($dlls -join ', ')", "Windows 10/11 physical-device acceptance: pending"
    ) -join "`n"
    [IO.File]::WriteAllText((Join-Path $package "BUILD-INFO.txt"), $buildInfo + "`n", [Text.UTF8Encoding]::new($false))
    $hashLines = @(Get-ChildItem -LiteralPath $package -File -Recurse | Sort-Object FullName | ForEach-Object {
        $relative = [IO.Path]::GetRelativePath($package, $_.FullName).Replace('\', '/')
        "$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $relative"
    })
    [IO.File]::WriteAllText((Join-Path $package "SHA256SUMS.txt"), ($hashLines -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
    Compress-Archive -LiteralPath $package -DestinationPath $archive -CompressionLevel Optimal
    $zipHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $output "SHA256SUMS.txt"), "$zipHash  $([IO.Path]::GetFileName($archive))`n", [Text.UTF8Encoding]::new($false))
    $unpacked = Join-Path $temporary "unpacked"
    Expand-Archive -LiteralPath $archive -DestinationPath $unpacked
    $verifiedRoot = Join-Path $unpacked "NekoSportsWorldTool-J-claude"
    foreach ($line in (Get-Content -LiteralPath (Join-Path $verifiedRoot "SHA256SUMS.txt"))) {
        $expectedHash, $relative = $line -split '  ', 2
        $file = Join-Path $verifiedRoot $relative
        if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -cne $expectedHash) { throw "Checksum mismatch: $relative" }
    }
    $actualFiles = @(Get-ChildItem -LiteralPath $verifiedRoot -File -Recurse)
    foreach ($file in $actualFiles) {
        $relative = [IO.Path]::GetRelativePath($verifiedRoot, $file.FullName).Replace('\', '/')
        if ($relative -notmatch '^(NekoSportsWorldTool\.exe|LICENSE|README-Windows\.md|使用说明-简体中文\.md|BUILD-INFO\.txt|SHA256SUMS\.txt|THIRD-PARTY-NOTICES\.txt|licenses/.+)$') { throw "Unexpected packaged file: $relative" }
        if ($relative -match '(?i)(session\.json|config\.json|identity\.json|\.pdb$|\.log$|\.exe\.(new|old)$)') { throw "Runtime/private data in package: $relative" }
    }
    $smokeResult = @(& (Join-Path $PSScriptRoot "smoke_windows.ps1") -Binary (Join-Path $verifiedRoot "NekoSportsWorldTool.exe") -AllowMissingOpenGLContext)
    $guiSmokeStatus = if ($smokeResult -contains "NSWT_GUI_SMOKE=passed") {
        "passed"
    } elseif ($smokeResult -contains "NSWT_GUI_SMOKE=unavailable_missing_opengl_2_context") {
        "unavailable_missing_opengl_2_context"
    } else {
        throw "Packaged executable smoke returned no recognized GUI status"
    }
    $evidence = [ordered]@{
        version = $version; commit = $commit; target = "x86_64-pc-windows-msvc";
        productName = $info.ProductName; comments = $info.Comments;
        signature = $signatureStatus; systemImports = $dlls;
        zipSHA256 = $zipHash; fileCount = $actualFiles.Count;
        guiSmoke = $guiSmokeStatus;
        windows10And11PhysicalAcceptance = "pending"
    }
    [IO.File]::WriteAllText((Join-Path $output "windows-build-evidence.json"), ($evidence | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
    Write-Host "Verified Windows x64 artifact: $([IO.Path]::GetFileName($archive))"
} finally {
    if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Recurse -Force }
}
