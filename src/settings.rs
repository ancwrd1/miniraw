use std::sync::atomic::{AtomicBool, Ordering};

use windows::{
    Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, REG_DWORD, RegCloseKey, RegCreateKeyW, RegOpenKeyW,
        RegQueryValueExW, RegSetKeyValueW,
    },
    core::PCWSTR,
};

use crate::{REG_KEY_NAME, REG_VALUE_NAME, utf16z};

pub struct AppSettings {
    discard_flag: AtomicBool,
}

impl AppSettings {
    pub fn load() -> Self {
        let mut discard_flag = false;

        unsafe {
            let mut hkey = HKEY::default();
            let key_name = utf16z!(REG_KEY_NAME);
            let value_name = utf16z!(REG_VALUE_NAME);
            if RegOpenKeyW(HKEY_CURRENT_USER, PCWSTR(key_name.as_ptr()), &mut hkey).is_ok() {
                let mut data = [0u8; 4];
                let mut size = data.len() as u32;
                if RegQueryValueExW(
                    hkey,
                    PCWSTR(value_name.as_ptr()),
                    None,
                    None,
                    Some(data.as_mut_ptr()),
                    Some(&mut size),
                )
                .is_ok()
                {
                    discard_flag = u32::from_ne_bytes(data) != 0;
                }
                let _ = RegCloseKey(hkey);
            }
        }

        Self {
            discard_flag: AtomicBool::new(discard_flag),
        }
    }

    pub fn store(&self) {
        unsafe {
            let mut hkey = HKEY::default();
            let key_name = utf16z!(REG_KEY_NAME);
            let value_name = utf16z!(REG_VALUE_NAME);
            let rc = RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(key_name.as_ptr()), &mut hkey);
            if rc.is_ok() {
                let data = (self.discard_flag.load(Ordering::SeqCst) as u32).to_ne_bytes();
                let _ = RegSetKeyValueW(
                    hkey,
                    PCWSTR::null(),
                    PCWSTR(value_name.as_ptr()),
                    REG_DWORD.0,
                    Some(data.as_ptr() as _),
                    data.len() as _,
                );
                let _ = RegCloseKey(hkey);
            }
        }
    }

    pub fn set_discard_flag(&self, discard_flag: bool) {
        self.discard_flag.store(discard_flag, Ordering::SeqCst);
    }

    pub fn discard_flag(&self) -> bool {
        self.discard_flag.load(Ordering::SeqCst)
    }
}
