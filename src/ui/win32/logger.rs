use log::{LevelFilter, Metadata, Record};
use widestring::{WideCStr, WideCString};
use winapi::{
    shared::{
        minwindef::{LPARAM, WPARAM},
        windef::HWND,
    },
    um::winuser::{SendMessageW, WM_GETTEXT, WM_GETTEXTLENGTH, WM_SETTEXT},
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
                let text_len = SendMessageW(self.0, WM_GETTEXTLENGTH, 0, 0) as usize;
                let mut buffer = vec![0u16; text_len + 2];

                let cur_len = SendMessageW(
                    self.0,
                    WM_GETTEXT,
                    buffer.len() as WPARAM,
                    buffer.as_mut_ptr() as LPARAM,
                );

                if cur_len >= 0 {
                    let old_text = WideCStr::from_slice_with_nul(&buffer)
                        .unwrap()
                        .to_string_lossy();

                    let time = time::OffsetDateTime::now_local();

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
                        time.millisecond(),
                        record.args()
                    );
                    let msg = WideCString::from_str(&msg).unwrap();

                    SendMessageW(self.0, WM_SETTEXT, 0, msg.as_ptr() as LPARAM);
                }
            }
        }
    }

    fn flush(&self) {}
}
