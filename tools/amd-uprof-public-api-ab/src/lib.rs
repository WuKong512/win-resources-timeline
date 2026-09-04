pub const API_DLL_NAME: &str = "AMDPowerProfileAPI.dll";
pub const STATIC_MAIN_REACHED: &str = "STATIC_FIXTURE_MAIN_REACHED=true";
pub const DYNAMIC_MAIN_REACHED: &str = "DYNAMIC_FIXTURE_MAIN_REACHED=true";
pub const LOAD_FLAGS: u32 = 0x0000_0100 | 0x0000_0800;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_markers_are_stable() {
        assert_eq!(STATIC_MAIN_REACHED, "STATIC_FIXTURE_MAIN_REACHED=true");
        assert_eq!(DYNAMIC_MAIN_REACHED, "DYNAMIC_FIXTURE_MAIN_REACHED=true");
    }

    #[test]
    fn dynamic_loader_uses_only_safe_directory_and_system32_search_flags() {
        assert_eq!(LOAD_FLAGS, 0x0000_0900);
    }

    #[test]
    fn public_api_dll_identity_is_explicit() {
        assert_eq!(API_DLL_NAME, "AMDPowerProfileAPI.dll");
    }
}
