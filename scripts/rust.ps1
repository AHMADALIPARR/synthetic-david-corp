param([ValidateSet('test','build','fmt','check')][string]$Action = 'test')
$ErrorActionPreference = 'Stop'
$projectDir = Split-Path -Parent $PSScriptRoot
$localCargoDir = Join-Path $projectDir '.tools/cargo'
$localRustupDir = Join-Path $projectDir '.tools/rustup'
if (Test-Path -LiteralPath (Join-Path $localCargoDir 'bin/cargo.exe')) {
    $env:CARGO_HOME = $localCargoDir
    $env:RUSTUP_HOME = $localRustupDir
    $compilerBin = Join-Path $projectDir '.tools/w64devkit/bin'
    $env:PATH = "$localCargoDir\bin;$compilerBin;$env:PATH"
    $gnuLinker = Join-Path $localRustupDir 'toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained/x86_64-w64-mingw32-gcc.exe'
    if (Test-Path -LiteralPath $gnuLinker) {
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $gnuLinker
        $env:RUSTFLAGS = '-C link-self-contained=yes'
    }
}
$manifest = Join-Path $projectDir 'rust/Cargo.toml'
if ($Action -eq 'fmt') { & cargo fmt --manifest-path $manifest --all }
else { & cargo $Action --manifest-path $manifest --locked -j 2 }
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
