param()
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

Push-Location $root
try {
    & (Join-Path $PSScriptRoot 'build-libc.ps1')
    if ($LASTEXITCODE -ne 0) { throw "C library build failed: $LASTEXITCODE" }
    & (Join-Path $PSScriptRoot 'build-c-program.ps1')
    if ($LASTEXITCODE -ne 0) { throw "C user program build failed: $LASTEXITCODE" }

    # GNU Bash is the initial Ring 3 process and replaces the former Rust shell.
    $sh = Get-Command sh -ErrorAction SilentlyContinue
    if (-not $sh) { throw "sh was not found; GNU Bash needs a POSIX shell to build" }
    & $sh.Source (Join-Path $PSScriptRoot 'prepare-bash53.sh')
    if ($LASTEXITCODE -ne 0) { throw "GNU Bash preparation failed: $LASTEXITCODE" }
    & $sh.Source (Join-Path $PSScriptRoot 'build-bash53.sh')
    if ($LASTEXITCODE -ne 0) { throw "GNU Bash build failed: $LASTEXITCODE" }

    cargo check -p sbos-runtime -p sbos-posix --target x86_64-unknown-none
    if ($LASTEXITCODE -ne 0) { throw "Cargo runtime build failed: $LASTEXITCODE" }
    foreach ($applet in @('ls','cat','clear','id','mv')) {
        & (Join-Path $PSScriptRoot 'build-c-program.ps1') -Source 'userland/applets.c' -Output "build/userland/$applet.elf"
        if ($LASTEXITCODE -ne 0) { throw "Building $applet failed: $LASTEXITCODE" }
    }
    cargo rustc -p sbos-kernel --release --target x86_64-unknown-none -- -C relocation-model=static -C link-arg=-no-pie -C link-arg=-Tkernel/linker.ld
    if ($LASTEXITCODE -ne 0) { throw "Kernel link failed: $LASTEXITCODE" }
    cargo build -p sbos-boot --release --target x86_64-unknown-uefi
    if ($LASTEXITCODE -ne 0) { throw "UEFI build failed: $LASTEXITCODE" }

    $esp = Join-Path $root 'build/esp'
    New-Item -ItemType Directory -Force -Path (Join-Path $esp 'EFI/BOOT') | Out-Null
    Copy-Item -Force (Join-Path $root 'target/x86_64-unknown-uefi/release/BOOTX64.efi') (Join-Path $esp 'EFI/BOOT/BOOTX64.EFI')
    Copy-Item -Force (Join-Path $root 'target/x86_64-unknown-none/release/kernel') (Join-Path $esp 'kernel.elf')
    Copy-Item -Force (Join-Path $root 'build/bash/bash.elf') (Join-Path $esp 'shell.elf')
    Set-Content -LiteralPath (Join-Path $esp 'startup.nsh') -Encoding Ascii -Value 'FS0:\EFI\BOOT\BOOTX64.EFI'
    & python (Join-Path $PSScriptRoot 'verify-artifacts.py') $esp
    if ($LASTEXITCODE -ne 0) { throw "artifact verification failed with exit code $LASTEXITCODE" }
    Write-Host "ESP directory prepared at $esp"
} finally {
    Pop-Location
}
