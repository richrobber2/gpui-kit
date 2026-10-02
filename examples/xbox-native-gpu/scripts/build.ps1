$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root
& "$PSScriptRoot\prepare.ps1"
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (!(Test-Path $vswhere)) { throw 'Visual Studio Build Tools are missing. Use the windows-2022 runner.' }
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.ComponentGroup.UWP.VC.v142 -property installationPath
if (!$vs) { throw 'Required VS component is missing: Microsoft.VisualStudio.ComponentGroup.UWP.VC.v142. Install it with the Universal Windows Platform development workload.' }
$msbuild = Join-Path $vs 'MSBuild\Current\Bin\MSBuild.exe'
$kitRoot = "${env:ProgramFiles(x86)}\Windows Kits\10"
$sdk = Get-ChildItem "$kitRoot\bin" -Directory | Where-Object { $_.Name -match '^10\.0\.\d+\.0$' -and [version]$_.Name -ge [version]'10.0.22621.0' -and (Test-Path "$($_.FullName)\x64\fxc.exe") -and (Test-Path "$kitRoot\Include\$($_.Name)\um\d3d11.h") } | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (!$sdk) { throw 'Windows SDK 22621 or newer with fxc.exe is required.' }
# Use the UWP Rust target, rather than linking a desktop Windows Rust library.
# Tier 3 requires nightly and rebuilding std from rust-src.
$devShell = Join-Path $vs 'Common7\Tools\Launch-VsDevShell.ps1'
& $devShell -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
$env:LIB = "$kitRoot\Lib\$($sdk.Name)\um\x64;$kitRoot\Lib\$($sdk.Name)\ucrt\x64;$env:LIB"
& cargo +nightly-2026-09-30 build --manifest-path "$root\ui\Cargo.toml" --release --locked -Z build-std=std,panic_unwind --target x86_64-uwp-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'GPUI Kit UWP Rust build failed.' }
# The preview wire format must reject partial writes and preserve color channels.
& rustc +nightly-2026-09-30 --edition 2024 --test "$root\ui\src\preview.rs" -o "$root\generated-model-preview-test.exe"
if ($LASTEXITCODE -ne 0) { throw 'Model preview validation did not compile.' }
& "$root\generated-model-preview-test.exe"
if ($LASTEXITCODE -ne 0) { throw 'Model preview validation failed.' }
# Run the existing independent CPU/reference validation before packaging.
& cl /nologo /EHsc /std:c++17 /Fe"$root\generated-matrix-test.exe" "$root\tests\matrix_test.cpp"
if ($LASTEXITCODE -ne 0) { throw 'Matrix validation did not compile.' }
& "$root\generated-matrix-test.exe"
if ($LASTEXITCODE -ne 0) { throw 'Matrix validation failed.' }
New-Item generated,artifacts -ItemType Directory -Force | Out-Null
& "$($sdk.FullName)\x64\fxc.exe" /nologo /T cs_5_0 /E main /O3 /Fh generated/MatmulShader.h /Vn g_matmulShader src/Matmul.hlsl
if ($LASTEXITCODE -ne 0) { throw 'HLSL compilation failed.' }
# Ephemeral, development-only certificate. Its private key is never uploaded.
$cert = $null
try {
    $cert = New-SelfSignedCertificate -Type Custom -Subject 'CN=XboxGpuLab' -FriendlyName 'Xbox GPU Lab development' -KeyUsage DigitalSignature -CertStoreLocation 'Cert:\CurrentUser\My' -NotAfter (Get-Date).AddMonths(12) -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3','2.5.29.19={text}')
    # Signing via certificate store avoids passing a password or secret on a command line.
    & $msbuild XboxGpu.vcxproj /m /t:Rebuild /p:Configuration=Release /p:Platform=x64 "/p:WindowsTargetPlatformVersion=$($sdk.Name)" /p:UapAppxPackageBuildMode=SideloadOnly /p:AppxBundle=Never /p:AppxPackageSigningEnabled=true "/p:PackageCertificateThumbprint=$($cert.Thumbprint)" "/p:AppxPackageDir=$root\artifacts\" /verbosity:minimal
    if ($LASTEXITCODE -ne 0) { throw 'Native app build/package failed. Read the MSBuild log above.' }
    # A backend can introduce imports even when it is not selected at runtime.
    $imports = & dumpbin /nologo /imports "$root\build\bin\XboxGpu.exe"
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect the native executable imports.' }
    if ($imports | Select-String -Pattern '(?i)\bopengl32\.dll\b') {
        throw 'Desktop OpenGL imports are unavailable on Xbox. Build WGPU with DX12 only for UWP.'
    }
    Write-Host 'Native package built; exporting development certificate.'
    Export-Certificate -Cert $cert -FilePath "$root\artifacts\XboxGpuLab.cer" | Out-Null
    Copy-Item "$root\INSTALL.md" "$root\artifacts\INSTALL.md"
    $packages = @(Get-ChildItem "$root\artifacts" -Recurse -File | Where-Object { $_.Name -like '*XboxGpu*' -and $_.Extension -in '.appx','.msix' })
    if (!$packages.Count) { throw 'Build returned no Xbox app package.' }
    # This UWP-only manifest uses the common APPX/MSIX container format.
    # Give the signed package the .appx extension for Xbox portal file pickers;
    # renaming does not alter signed contents. Verify the renamed file below.
    $packages = @($packages | ForEach-Object {
        if ($_.Extension -eq '.msix') {
            $destination = [System.IO.Path]::ChangeExtension($_.FullName, '.appx')
            Move-Item -LiteralPath $_.FullName -Destination $destination
            Get-Item -LiteralPath $destination
        } else { $_ }
    })
    # Trust only this newly generated certificate in the disposable runner while verifying.
    Write-Host 'Verifying signature with temporary certificate trust on the disposable runner.'
    Import-Certificate -FilePath "$root\artifacts\XboxGpuLab.cer" -CertStoreLocation Cert:\LocalMachine\Root | Out-Null
    foreach ($package in $packages) {
        Write-Host "Verifying $($package.Name)"
        & "$($sdk.FullName)\x64\signtool.exe" verify /pa /v $package.FullName
        if ($LASTEXITCODE -ne 0) { throw 'Package signature verification failed.' }
    }
    Get-ChildItem "$root\artifacts" -Recurse -File | Where-Object { $_.Extension -eq '.appx' } | ForEach-Object { Get-FileHash $_.FullName -Algorithm SHA256 } | Format-Table | Out-String | Set-Content "$root\artifacts\SHA256.txt"
    Write-Host "Installable packages are in $root\artifacts"
} finally {
    if ($cert) {
        Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -Force -ErrorAction SilentlyContinue
        Remove-Item "Cert:\LocalMachine\Root\$($cert.Thumbprint)" -Force -ErrorAction SilentlyContinue
    }
}
