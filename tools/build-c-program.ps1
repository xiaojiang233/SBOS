param(
    [string]$Source = 'userland/posix-probe.c',
    [string]$Output = 'build/userland/posix-probe.elf'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$clang = (Get-Command clang -ErrorAction Stop).Source
$sourcePath = [System.IO.Path]::GetFullPath((Join-Path $root $Source))
$outputPath = [System.IO.Path]::GetFullPath((Join-Path $root $Output))
$outputDirectory = Split-Path -Parent $outputPath
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$objectPath = Join-Path $outputDirectory 'program.o'

& $clang --target=x86_64-unknown-none-elf -std=c11 -O2 -ffreestanding `
    -fno-builtin -fno-stack-protector -fno-pic -mno-red-zone `
    -Wall -Wextra -I (Join-Path $root 'libc/include') `
    -I (Join-Path $root 'user/runtime/include') `
    -c $sourcePath -o $objectPath
if ($LASTEXITCODE -ne 0) { throw "C user program compilation failed: $Source" }

& $clang --target=x86_64-unknown-none-elf -fuse-ld=lld -nostdlib -static `
    '-Wl,--build-id=none' '-Wl,-z,max-page-size=0x1000' `
    ('-Wl,-T,' + (Join-Path $root 'user/runtime/linker.ld')) `
    (Join-Path $root 'build/libc/crt0.o') $objectPath `
    (Join-Path $root 'build/libc/libsbos.a') -o $outputPath
if ($LASTEXITCODE -ne 0) { throw "C user program link failed: $Source" }
Write-Host "C ELF built at $outputPath"
