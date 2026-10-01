$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Split-Path $PSScriptRoot -Parent
New-Item "$root\vendor" -ItemType Directory -Force | Out-Null
# Keep upstream sources out of git; apply small, reviewable patches to pinned releases.
$crates = @(
    @{ Name = 'gpui-pre-wgpu'; Version = '0.3.7' },
    @{ Name = 'wgpu-hal'; Version = '29.0.4' }
)
foreach ($crate in $crates) {
    $destination = Join-Path "$root\vendor" $crate.Name
    if (Test-Path $destination) {
        # An existing tree must already contain exactly our patch.
        & git -C $destination apply --reverse --check "$root\patches\$($crate.Name).patch"
        if ($LASTEXITCODE -ne 0) { throw "Existing vendor tree is not patched: $destination" }
        continue
    }
    $archive = Join-Path $env:TEMP "$($crate.Name)-$($crate.Version).crate"
    Invoke-WebRequest "https://static.crates.io/crates/$($crate.Name)/$($crate.Name)-$($crate.Version).crate" -OutFile $archive
    & tar -xf $archive -C "$root\vendor"
    if ($LASTEXITCODE -ne 0) { throw 'Could not extract dependency.' }
    Move-Item "$root\vendor\$($crate.Name)-$($crate.Version)" $destination
    & git -C $destination apply --check "$root\patches\$($crate.Name).patch"
    if ($LASTEXITCODE -ne 0) { throw 'Dependency patch does not match the pinned source.' }
    & git -C $destination apply "$root\patches\$($crate.Name).patch"
    if ($LASTEXITCODE -ne 0) { throw 'Dependency patch failed.' }
}
foreach ($source in Get-ChildItem "$root\Assets\*.png.base64") {
    $destination = $source.FullName.Substring(0, $source.FullName.Length - '.base64'.Length)
    [IO.File]::WriteAllBytes($destination, [Convert]::FromBase64String([IO.File]::ReadAllText($source.FullName)))
}
