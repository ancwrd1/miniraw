use log::{LevelFilter, Metadata, Record};
use time::OffsetDateTime;

use crate::ui::window::WindowRef;

pub struct WindowLogger(WindowRef);
unsafe impl Send for WindowLogger {}
unsafe impl Sync for WindowLogger {}

impl WindowLogger {
    pub fn init(win: WindowRef, level: LevelFilter) {
        let _ = log::set_boxed_logger(Box::new(WindowLogger(win)));
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
            let old_text = self.0.get_text().unwrap_or_default();
            let time = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
            let (hour, minute, second, nano) = time.to_hms_nano();

            let msg = format!(
                "{}[{}] {}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {}\r\n",
                old_text,
                record.level(),
                time.year(),
                time.month() as u8 + 1,
                time.day(),
                hour,
                minute,
                second,
                nano / 1_000_000,
                record.args()
            );
            let _ = self.0.set_text(&msg);
        }
    }

    fn flush(&self) {}
}
