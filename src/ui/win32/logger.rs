use chrono::{Datelike, Timelike};
use log::{LevelFilter, Metadata, Record};
use widestring::{U16CStr, U16CString};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{SendMessageW, WM_GETTEXT, WM_GETTEXTLENGTH, WM_SETTEXT},
};

pub struct WindowLogger(HWND);
unsafe impl Send for WindowLogger {}
unsafe impl Sync for WindowLogger {}

impl WindowLogger {
    pub fn init(hwnd: HWND, level: LevelFilter) {
        let _ = log::set_boxed_logger(Box::new(WindowLogger(hwnd)));
        log::set_max_level(level);
    }

    fn is_our_path(&self, path: &Option<&str>) -> bool {
        path.iter().any(|p| p.starts_with("miniraw"))
    }
}

impl log::Log for WindowLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) && self.is_our_path(&record.module_path()) {
            unsafe {
                let text_len = SendMessageW(
                    self.0,
                    WM_GETTEXTLENGTH,
                    WPARAM::default(),
                    LPARAM::default(),
                )
                .0 as usize;
                let mut buffer = vec![0u16; text_len + 2];

                let cur_len = SendMessageW(
                    self.0,
                    WM_GETTEXT,
                    WPARAM(buffer.len() as _),
                    LPARAM(buffer.as_mut_ptr() as _),
                );

                if cur_len.0 >= 0 {
                    let old_text = U16CStr::from_slice_unchecked(&buffer).to_string_lossy();

                    let time = chrono::Local::now();

                    let msg = format!(
                        "{}[{}] {}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {}\r\n",
                        old_text,
                        record.level(),
                        time.year(),
                        time.month(),
                        time.day(),
                        time.hour(),
                        time.minute(),
                        time.second(),
                        time.nanosecond() / 1_000_000,
                        record.args()
                    );
                    let msg = U16CString::from_str_unchecked(&msg);

                    SendMessageW(
                        self.0,
                        WM_SETTEXT,
                        WPARAM::default(),
                        LPARAM(msg.as_ptr() as _),
                    );
                }
            }
        }
    }

    fn flush(&self) {}
}
