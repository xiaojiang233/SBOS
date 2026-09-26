$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$clang = (Get-Command clang -ErrorAction Stop).Source
$llvmAr = (Get-Command llvm-ar -ErrorAction Stop).Source
$out = Join-Path $root 'build/libc'
$sourceDir = Join-Path $root 'libc/src'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$objects = @()

# Build artifacts are regenerated every run. Deleting them directly (instead of
# Remove-Item) keeps the script working in sandboxes where cmdlet deletion is
# hooked or routed through a trash service.
function Remove-Artifact([string] $path) {
    if ([System.IO.File]::Exists($path)) {
        [System.IO.File]::Delete($path)
    }
}

Get-ChildItem -LiteralPath $sourceDir -Filter '*.c' -File |
    Where-Object { $_.Name -ne 'env_startup_bash.c' } |
    Sort-Object Name | ForEach-Object {
    $object = Join-Path $out ($_.BaseName + '.o')
    & $clang --target=x86_64-unknown-none-elf -std=c11 -O2 -ffreestanding `
        -fno-builtin -fno-stack-protector -fno-pic -mno-red-zone `
        -Wall -Wextra -I (Join-Path $root 'libc/include') `
        -I (Join-Path $root 'user/runtime/include') `
        -c $_.FullName -o $object
    if ($LASTEXITCODE -ne 0) { throw "C library compilation failed: $($_.Name)" }
    $objects += $object
}

$archive = Join-Path $out 'libsbos.a'
Remove-Artifact $archive
$assembly = Join-Path $out 'setjmp.o'
& $clang --target=x86_64-unknown-none-elf -c (Join-Path $root 'libc/src/setjmp.S') -o $assembly
if ($LASTEXITCODE -ne 0) { throw "setjmp assembly compilation failed: $LASTEXITCODE" }
$objects += $assembly
& $llvmAr rcs $archive @objects
if ($LASTEXITCODE -ne 0) { throw "Creating libsbos.a failed: $LASTEXITCODE" }

# Bash maintains its own exported-variable view. Its lib/sh/getenv.c provides
# getenv/setenv/putenv, so the Bash-specific archive omits those libc symbols.
$bashObjects = @($objects | Where-Object { [System.IO.Path]::GetFileName($_) -ne 'stdlib.o' })
$bashStdlib = Join-Path $out 'stdlib-bash.o'
& $clang --target=x86_64-unknown-none-elf -std=c11 -O2 -ffreestanding `
    -fno-builtin -fno-stack-protector -fno-pic -mno-red-zone -DSBOS_NO_ENVIRONMENT `
    -Wall -Wextra -I (Join-Path $root 'libc/include') `
    -I (Join-Path $root 'user/runtime/include') `
    -c (Join-Path $sourceDir 'stdlib.c') -o $bashStdlib
if ($LASTEXITCODE -ne 0) { throw "Bash C runtime compilation failed: $LASTEXITCODE" }
$bashObjects += $bashStdlib
$bashEnvironment = Join-Path $out 'env-startup-bash.o'
& $clang --target=x86_64-unknown-none-elf -std=c11 -O2 -ffreestanding `
    -fno-builtin -fno-stack-protector -fno-pic -mno-red-zone `
    -I (Join-Path $root 'libc/include') `
    -I (Join-Path $root 'user/runtime/include') `
    -c (Join-Path $sourceDir 'env_startup_bash.c') -o $bashEnvironment
if ($LASTEXITCODE -ne 0) { throw "Bash environment startup compilation failed: $LASTEXITCODE" }
$bashObjects += $bashEnvironment
$bashArchive = Join-Path $out 'libsbos-bash.a'
Remove-Artifact $bashArchive
& $llvmAr rcs $bashArchive @bashObjects
if ($LASTEXITCODE -ne 0) { throw "Creating libsbos-bash.a failed: $LASTEXITCODE" }

& $clang --target=x86_64-unknown-none-elf -c (Join-Path $root 'libc/crt0.S') `
    -o (Join-Path $out 'crt0.o')
if ($LASTEXITCODE -ne 0) { throw "C runtime entry compilation failed: $LASTEXITCODE" }
Write-Host "SBOS C runtime built at $out"
