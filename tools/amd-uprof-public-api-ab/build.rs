use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=AMD_UPROF_ROOT");

    let Some(root) = env::var_os("AMD_UPROF_ROOT") else {
        println!(
            "cargo:warning=AMD_UPROF_ROOT is unset; static fixture linking requires the installed uProf root"
        );
        return;
    };

    let root = PathBuf::from(root);
    if !root.is_absolute() {
        panic!("AMD_UPROF_ROOT must be an absolute path");
    }

    let bin = root.join("bin");
    let import_library = bin.join("AMDPowerProfileAPI.lib");
    if !import_library.is_file() {
        panic!(
            "official AMDPowerProfileAPI import library is missing: {}",
            import_library.display()
        );
    }

    println!("cargo:rustc-link-search=native={}", bin.display());
    println!("cargo:rerun-if-changed={}", import_library.display());
}
