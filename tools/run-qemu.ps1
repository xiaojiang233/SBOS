param(
    [string]$OvmfCode = $env:OVMF_CODE,
    [string]$Display = 'sdl',
    [switch]$Debug
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$localQemu = Join-Path $root '_qemu/qemu/qemu-system-x86_64.exe'
$qemu = Get-Command qemu-system-x86_64 -ErrorAction SilentlyContinue
if ($qemu) { $qemuExe = $qemu.Source }
elseif (Test-Path -LiteralPath $localQemu) { $qemuExe = $localQemu }
else { throw 'qemu-system-x86_64 is not installed. Run the QEMU setup under _qemu first.' }

$qemuRoot = Split-Path -Parent $qemuExe
if (-not $OvmfCode) {
    $candidates = @(
        (Join-Path $root '_qemu/firmware/OVMF_CODE_4M.fd'),
        (Join-Path $qemuRoot 'share/qemu/edk2-x86_64-code.fd'),
        (Join-Path $qemuRoot 'share/qemu/OVMF_CODE.fd')
    )
    $OvmfCode = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (-not $OvmfCode -or -not (Test-Path -LiteralPath $OvmfCode)) {
    throw 'Set OVMF_CODE to an x86_64 OVMF firmware image; none was found beside QEMU.'
}

$esp = Join-Path $root 'build/esp'
if (-not (Test-Path (Join-Path $esp 'EFI/BOOT/BOOTX64.EFI'))) { throw 'Run tools/build.ps1 first.' }
$varsTemplate = Join-Path $root '_qemu/firmware/OVMF_VARS_4M.fd'
$varsPath = Join-Path $root 'build/OVMF_VARS.fd'
if ((Test-Path -LiteralPath $varsTemplate) -and -not (Test-Path -LiteralPath $varsPath)) {
    Copy-Item -LiteralPath $varsTemplate -Destination $varsPath
}
$env:QEMU_DATA_DIR = Join-Path $qemuRoot 'share'
$disk = Join-Path $root 'build/sbfs.img'
if (-not (Test-Path -LiteralPath $disk)) {
    $stream = [System.IO.File]::Open($disk, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
    try { $stream.SetLength(64MB) } finally { $stream.Dispose() }
}
$qemuArgs = @('-machine','pc','-cpu','qemu64','-m','512M', '-netdev','user,id=net0', '-device','e1000,netdev=net0,mac=52:54:00:12:34:56')
if (Test-Path -LiteralPath $varsPath) {
    $qemuArgs += @('-drive',"if=pflash,format=raw,unit=0,file=$OvmfCode,readonly=on")
    $qemuArgs += @('-drive',"if=pflash,format=raw,unit=1,file=$varsPath")
} else {
    $qemuArgs += @('-bios',$OvmfCode)
}
$qemuArgs += @('-drive',"if=ide,index=0,format=raw,file=fat:rw:$esp",'-drive',"if=ide,index=1,format=raw,file=$disk",'-boot','order=c','-serial','stdio','-monitor','none','-display',$Display,'-no-reboot')
if ($Debug) { $qemuArgs += @('-no-shutdown','-d','int,guest_errors,cpu_reset','-D',(Join-Path $root 'build/qemu-debug.log')) }
& $qemuExe @qemuArgs
if ($LASTEXITCODE -ne 0) { throw "QEMU exited with status $LASTEXITCODE" }
