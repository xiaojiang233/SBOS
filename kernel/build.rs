use std::env;
use std::fs;
use std::path::PathBuf;

/// User programs that the kernel embeds and installs into /Applications.
const APPLICATIONS: &[(&str, &str)] = &[
    ("posix-probe.elf", "build/userland/posix-probe.elf"),
    ("coreutils.elf", "build/userland/coreutils.elf"),
    ("clear.elf", "build/userland/clear.elf"),
    ("id.elf", "build/userland/id.elf"),
    ("mv.elf", "build/userland/mv.elf"),
    ("fault.elf", "build/userland/fault.elf"),
    ("channel-probe.elf", "build/userland/channel-probe.elf"),
    ("network-probe.elf", "build/userland/network-probe.elf"),
];

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let workspace = manifest.parent().unwrap();
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for (name, relative) in APPLICATIONS {
        let source = workspace.join(relative);
        let output = out_dir.join(name);
        println!("cargo:rerun-if-changed={}", source.display());
        if source.is_file() {
            fs::copy(&source, &output)
                .unwrap_or_else(|_| panic!("copy {} into kernel build output", relative));
        } else {
            // Nothing to install: the kernel checks for an ELF header and skips
            // the entry instead of failing the build.
            fs::write(&output, b"").unwrap_or_else(|_| panic!("write empty fallback for {}", name));
        }
    }
}
