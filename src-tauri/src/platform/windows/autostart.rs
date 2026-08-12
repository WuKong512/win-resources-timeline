use std::{io, os::windows::process::CommandExt, path::Path, process::Command};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn refresh_command() -> io::Result<()> {
    let executable = std::env::current_exe()?;
    write_run_value(&executable)
}

fn write_run_value(executable: &Path) -> io::Result<()> {
    let value = run_value(executable);
    let status = Command::new("reg.exe")
        .args([
            "ADD",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "Resource Timeline",
            "/t",
            "REG_SZ",
            "/d",
            &value,
            "/f",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "reg.exe exited with status {status}"
        )))
    }
}

fn run_value(executable: &Path) -> String {
    format!("\"{}\" --background", executable.display())
}

#[cfg(test)]
mod tests {
    use super::run_value;

    #[test]
    fn quotes_executable_paths_with_spaces() {
        let value = run_value(std::path::Path::new(
            r"F:\Apps\Resource Timeline Portable.exe",
        ));
        assert_eq!(
            value,
            r#""F:\Apps\Resource Timeline Portable.exe" --background"#
        );
    }
}
