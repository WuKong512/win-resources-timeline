use std::{
    ffi::{c_void, OsString},
    io::Write,
    os::windows::ffi::OsStrExt,
    path::PathBuf,
    ptr,
};

use amd_uprof_public_api_ab::{API_DLL_NAME, DYNAMIC_MAIN_REACHED, LOAD_FLAGS};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(
        library_file_name: *const u16,
        file_handle: *mut c_void,
        flags: u32,
    ) -> *mut c_void;

    fn GetLastError() -> u32;
}

fn wide_path(path: &std::path::Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn api_path() -> Result<PathBuf, String> {
    let mut args = std::env::args_os();
    let _program = args.next();
    let Some(path) = args.next() else {
        return Err(format!(
            "usage: amd-uprof-dynamic-api-load-fixture <absolute {API_DLL_NAME} path>"
        ));
    };
    if args.next().is_some() {
        return Err("exactly one DLL path argument is required".to_string());
    }

    let path = PathBuf::from(OsString::from(path));
    if !path.is_absolute() {
        return Err("DLL path must be absolute".to_string());
    }
    Ok(path)
}

fn main() {
    println!("{DYNAMIC_MAIN_REACHED}");

    let path = match api_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("DYNAMIC_FIXTURE_ARGUMENT_ERROR={error}");
            std::process::exit(2);
        }
    };
    let wide = wide_path(&path);

    println!("BEFORE_LOADLIBRARY=true");
    println!("DYNAMIC_FIXTURE_DLL_PATH={}", path.display());
    let _ = std::io::stdout().flush();

    let module = unsafe { LoadLibraryExW(wide.as_ptr(), ptr::null_mut(), LOAD_FLAGS) };
    if module.is_null() {
        let error = unsafe { GetLastError() };
        println!(
            "LOAD_RETURNED_ERROR win32_error={} win32_error_hex=0x{:08X}",
            error, error
        );
        std::process::exit(1);
    }

    println!("AFTER_LOADLIBRARY=true");
    println!("DYNAMIC_FIXTURE_MODULE_HANDLE=0x{:X}", module as usize);
}
