param(
    [Parameter(Mandatory = $true)][string]$Session,
    [string]$Log = '',
    [int]$BootSeconds = 10,
    [int]$RunSeconds = 25,
    [int]$CharacterDelayMilliseconds = 20,
    [int]$LinePauseMilliseconds = 350,
    [string]$Disk = '',
    [string]$HostForwardTcp = '',
    [int]$QmpPort = 0,
    [switch]$InterruptLog
)
# Run the SBOS image in QEMU headless, type a session script at the serial
# console, and capture the transcript.
#
# The session file holds one line per thing to type. The characters are written
# to QEMU's stdin, which the kernel now reads as serial console input (see
# drivers/serial.rs and drivers/tty.rs). Everything the kernel, the shell and
# the programs print comes back on the serial line and lands in the log.
#
# The script refuses to start when another qemu-system-x86_64 is running, so it
# never disturbs a QEMU session that was started by hand, and it uses its own
# disk image by default so the normal build/sbfs.img volume is left alone.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

# A QEMU instance started by hand is left completely alone: this script only
# ever stops the process it started itself, and it uses its own disk image and
# its own UEFI variable store so it cannot interfere with that session.
$existing = Get-Process -Name 'qemu-system-x86_64' -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "qemu-session: another QEMU is already running (PID $($existing.Id -join ', ')); leaving it untouched and using private files."
}

$localQemu = Join-Path $root '_qemu/qemu/qemu-system-x86_64.exe'
$qemuCommand = Get-Command qemu-system-x86_64 -ErrorAction SilentlyContinue
if ($qemuCommand) { $qemuExe = $qemuCommand.Source }
elseif (Test-Path -LiteralPath $localQemu) { $qemuExe = $localQemu }
else { throw 'qemu-system-x86_64 is not installed. Run the QEMU setup under _qemu first.' }

$ovmf = Join-Path $root '_qemu/firmware/OVMF_CODE_4M.fd'
$varsTemplate = Join-Path $root '_qemu/firmware/OVMF_VARS_4M.fd'
if (-not (Test-Path -LiteralPath $ovmf)) { throw "OVMF firmware not found at $ovmf" }
if (-not (Test-Path -LiteralPath $varsTemplate)) { throw "OVMF variables template not found at $varsTemplate" }

$esp = Join-Path $root 'build/esp'
if (-not (Test-Path (Join-Path $esp 'EFI/BOOT/BOOTX64.EFI'))) { throw 'Run tools/build.ps1 first.' }

$sessionPath = [System.IO.Path]::GetFullPath((Join-Path $root $Session))
if (-not (Test-Path -LiteralPath $sessionPath)) { throw "session script not found: $sessionPath" }

if (-not $Log) { $Log = Join-Path $root 'build/session-transcript.log' }
$logPath = [System.IO.Path]::GetFullPath($Log)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $logPath) | Out-Null
# Start from nothing so a stale transcript can never be mistaken for this run.
if (Test-Path -LiteralPath $logPath) { [System.IO.File]::Delete($logPath) }

if (-not $Disk) { $Disk = 'build/qemu-session/sbfs.img' }
$diskPath = [System.IO.Path]::GetFullPath((Join-Path $root $Disk))
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $diskPath) | Out-Null
if (-not (Test-Path -LiteralPath $diskPath)) {
    $stream = [System.IO.File]::Open($diskPath, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
    try { $stream.SetLength(64MB) } finally { $stream.Dispose() }
}

# Private copy of the UEFI variable store: two QEMU processes must not share it,
# and the session must not disturb the variable store used by a manual session.
$varsPath = Join-Path (Split-Path -Parent $diskPath) 'OVMF_VARS.fd'
if (-not (Test-Path -LiteralPath $varsPath)) {
    Copy-Item -LiteralPath $varsTemplate -Destination $varsPath
}

$env:QEMU_DATA_DIR = Join-Path (Split-Path -Parent $qemuExe) 'share'

$networkBackend = 'user,id=net0'
if ($HostForwardTcp) {
    if ($HostForwardTcp -notmatch '^(\d{1,5}):(\d{1,5})$') {
        throw 'HostForwardTcp must be written as hostPort:guestPort.'
    }
    $hostPort = [int]$Matches[1]
    $guestPort = [int]$Matches[2]
    if ($hostPort -lt 1 -or $hostPort -gt 65535 -or $guestPort -lt 1 -or $guestPort -gt 65535) {
        throw 'HostForwardTcp ports must be in the range 1..65535.'
    }
    $networkBackend += ",hostfwd=tcp:127.0.0.1:$hostPort-:$guestPort"
}

if ($QmpPort -lt 0 -or $QmpPort -gt 65535) { throw 'QmpPort must be 0 or in the range 1..65535.' }

$arguments = @(
    '-machine', 'pc', '-cpu', 'qemu64', '-m', '512M',
    '-netdev', $networkBackend,
    '-device', 'e1000,netdev=net0,mac=52:54:00:12:34:56',
    '-drive', "if=pflash,format=raw,unit=0,file=$ovmf,readonly=on",
    '-drive', "if=pflash,format=raw,unit=1,file=$varsPath",
    '-drive', "if=ide,index=0,format=raw,file=fat:rw:$esp",
    '-drive', "if=ide,index=1,format=raw,file=$diskPath",
    '-boot', 'order=c',
    '-serial', 'stdio',
    '-monitor', 'none',
    '-display', 'none',
    '-no-reboot'
)

if ($InterruptLog) {
    $arguments += @('-d', 'int,guest_errors', '-D', (Join-Path $root 'build/session-qemu-debug.log'))
}
if ($QmpPort -ne 0) {
    $arguments += @('-qmp', "tcp:127.0.0.1:$QmpPort,server=on,wait=off")
}

Write-Host "qemu-session: booting (waiting $BootSeconds s before typing)"
$startInfo = [System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $qemuExe
$startInfo.Arguments = ($arguments | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } }) -join ' '
$startInfo.RedirectStandardInput = $true
$startInfo.RedirectStandardOutput = $true
$startInfo.RedirectStandardError = $true
$startInfo.UseShellExecute = $false
$startInfo.CreateNoWindow = $true
$startInfo.StandardInputEncoding = [System.Text.Encoding]::ASCII
$startInfo.StandardOutputEncoding = [System.Text.Encoding]::ASCII

$process = [System.Diagnostics.Process]::new()
$process.StartInfo = $startInfo

$process.Start()

Start-Sleep -Seconds $BootSeconds

# Pace console input and leave time after each command for a newly spawned
# foreground process to take control of the terminal before the next line.
$text = [System.IO.File]::ReadAllText($sessionPath)
Write-Host "qemu-session: typing $($text.Length) characters"
for ($index = 0; $index -lt $text.Length; $index++) {
    $process.StandardInput.Write($text.Substring($index, 1))
    $process.StandardInput.Flush()
    if ($text[$index] -eq "`n") {
        Start-Sleep -Milliseconds $LinePauseMilliseconds
    } elseif ($CharacterDelayMilliseconds -gt 0) {
        Start-Sleep -Milliseconds $CharacterDelayMilliseconds
    }
}

Write-Host "qemu-session: letting the session run for $RunSeconds s"
Start-Sleep -Seconds $RunSeconds

if (-not $process.HasExited) {
    Stop-Process -Id $process.Id -Force
}
$process.WaitForExit(5000) | Out-Null

# Read the serial output only after QEMU is gone. The event based reader used
# earlier dropped data under load, which made short transcripts look like
# guest hangs. Reading the pipe to the end after exit is deterministic.
$stdout = $process.StandardOutput.ReadToEnd()
$stderr = $process.StandardError.ReadToEnd()
[System.IO.File]::WriteAllText($logPath, $stdout + $stderr)
Write-Host "qemu-session: transcript at $logPath ($($stdout.Length + $stderr.Length) characters)"
