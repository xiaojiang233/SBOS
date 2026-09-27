param(
    [string[]]$KernelFeatures = @('qemu')
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

Push-Location $root
try {
    & (Join-Path $PSScriptRoot 'build-libc.ps1')
    if ($LASTEXITCODE -ne 0) { throw "C library build failed: $LASTEXITCODE" }
    & (Join-Path $PSScriptRoot 'build-c-program.ps1')
    if ($LASTEXITCODE -ne 0) { throw "C user program build failed: $LASTEXITCODE" }

    $driveLetter = $root.Substring(0, 1).ToLowerInvariant()
    $linuxRoot = "/mnt/$driveLetter/$($root.Substring(3).Replace('\','/'))"
    # GNU Bash is the initial Ring 3 process and replaces the former Rust shell.
    & wsl.exe -e bash -lc "cd '$linuxRoot' && sh tools/prepare-bash53.sh"
    if ($LASTEXITCODE -ne 0) { throw "GNU Bash preparation failed: $LASTEXITCODE" }
    & wsl.exe -e bash -lc "cd '$linuxRoot' && sh tools/build-bash53.sh"
    if ($LASTEXITCODE -ne 0) { throw "GNU Bash build failed: $LASTEXITCODE" }

    & wsl.exe -e bash -lc "cd '$linuxRoot' && sh tools/build-coreutils.sh"
    if ($LASTEXITCODE -ne 0) { throw "GNU Coreutils build failed: $LASTEXITCODE" }
    Copy-Item -Force (Join-Path $root '_qemu/src/coreutils-9.12/wsl-build/src/coreutils') `
        (Join-Path $root 'build/userland/coreutils.elf')

    cargo check -p sbos-runtime -p sbos-posix --target x86_64-unknown-none
    if ($LASTEXITCODE -ne 0) { throw "Cargo runtime build failed: $LASTEXITCODE" }
    foreach ($applet in @('clear','id','mv')) {
        & (Join-Path $PSScriptRoot 'build-c-program.ps1') -Source 'userland/applets.c' -Output "build/userland/$applet.elf"
        if ($LASTEXITCODE -ne 0) { throw "Building $applet failed: $LASTEXITCODE" }
    }
    & (Join-Path $PSScriptRoot 'build-c-program.ps1') -Source 'userland/fault.c' -Output 'build/userland/fault.elf'
    if ($LASTEXITCODE -ne 0) { throw "Building fault failed: $LASTEXITCODE" }
    & (Join-Path $PSScriptRoot 'build-c-program.ps1') -Source 'userland/channel-probe.c' -Output 'build/userland/channel-probe.elf'
    if ($LASTEXITCODE -ne 0) { throw "Building channel-probe failed: $LASTEXITCODE" }
    & (Join-Path $PSScriptRoot 'build-c-program.ps1') -Source 'userland/network-probe.c' -Output 'build/userland/network-probe.elf'
    if ($LASTEXITCODE -ne 0) { throw "Building network-probe failed: $LASTEXITCODE" }
    & (Join-Path $PSScriptRoot 'build-c-program.ps1') -Source 'userland/desktop-demo.c' -Output 'build/userland/desktop-demo.elf'
    if ($LASTEXITCODE -ne 0) { throw "Building desktop-demo failed: $LASTEXITCODE" }
    & (Join-Path $PSScriptRoot 'build-c-program.ps1') -Source 'userland/mouse-probe.c' -Output 'build/userland/mouse-probe.elf'
    if ($LASTEXITCODE -ne 0) { throw "Building mouse-probe failed: $LASTEXITCODE" }
    & (Join-Path $PSScriptRoot 'build-c-program.ps1') -Source 'userland/tcp-listen-probe.c' -Output 'build/userland/tcp-listen-probe.elf'
    if ($LASTEXITCODE -ne 0) { throw "Building tcp-listen-probe failed: $LASTEXITCODE" }
    & (Join-Path $PSScriptRoot 'build-c-program.ps1') -Source 'userland/xserver.c' -Output 'build/userland/xserver.elf'
    if ($LASTEXITCODE -ne 0) { throw "Building xserver failed: $LASTEXITCODE" }
    & (Join-Path $PSScriptRoot 'build-c-program.ps1') -Source 'userland/select-probe.c' -Output 'build/userland/select-probe.elf'
    if ($LASTEXITCODE -ne 0) { throw "Building select-probe failed: $LASTEXITCODE" }
    if ($KernelFeatures.Count -eq 0) { throw 'Enable at least one kernel Cargo feature.' }
    $featureList = $KernelFeatures -join ','
    cargo rustc -p sbos-kernel --release --target x86_64-unknown-none --no-default-features --features $featureList -- -C relocation-model=static -C link-arg=-no-pie -C link-arg=-Tkernel/linker.ld
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
