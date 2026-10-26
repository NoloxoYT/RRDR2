/// Export the `rd_*` C ABI for a [`Mod`](crate::Mod) implementation.
///
/// Invoke this once at the crate root of the `cdylib`:
///
/// ```ignore
/// use rrdr_bridge::export_mod;
/// export_mod!(MyMod);
/// ```
#[macro_export]
macro_rules! export_mod {
    ($ty:ty) => {
        static RD_STATE: ::std::sync::Mutex<Option<$crate::ffi::State<$ty>>> =
            ::std::sync::Mutex::new(None);

        #[no_mangle]
        pub extern "C" fn rd_abi_version() -> u32 {
            $crate::ffi::ABI_VERSION
        }

        #[no_mangle]
        pub unsafe extern "C" fn rd_init(
            host: *const $crate::ffi::RdHost,
            config_path: *const ::std::ffi::c_char,
        ) -> i32 {
            unsafe { $crate::ffi::init(&RD_STATE, host, config_path) }
        }

        #[no_mangle]
        pub unsafe extern "C" fn rd_init_v3(
            host: *const $crate::ffi::RdHostV3,
            config_path: *const ::std::ffi::c_char,
        ) -> i32 {
            unsafe { $crate::ffi::init_v3(&RD_STATE, host, config_path) }
        }

        #[no_mangle]
        pub extern "C" fn rd_tick(dt: f32) {
            $crate::ffi::tick(&RD_STATE, dt)
        }

        #[no_mangle]
        pub extern "C" fn rd_shutdown() {
            $crate::ffi::shutdown(&RD_STATE)
        }

        #[no_mangle]
        pub unsafe extern "C" fn rd_reload_config(config_path: *const ::std::ffi::c_char) -> i32 {
            unsafe { $crate::ffi::reload(&RD_STATE, config_path) }
        }
    };
}
