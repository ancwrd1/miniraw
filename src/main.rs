#![windows_subsystem = "windows"]

use std::sync::Arc;

use log::{LevelFilter, error, info};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::{
    settings::AppSettings,
    ui::{
        MessageLoop,
        window::{
            Font, MessageResult, WindowBuilder, WindowError, WindowGeometry, WindowMessage,
            WindowMessageHandler, WindowRef,
        },
    },
};

pub mod listener;
pub mod logger;
pub mod settings;
pub mod ui;
pub mod util;

const IDI_MAINICON: u32 = 1000;
const IDM_DISCARD_FILES: u32 = 1001;
const REG_KEY_NAME: &str = "Software\\MiniRAW NG";
const REG_VALUE_NAME: &str = "discard";

struct MainWindow {
    settings: Arc<AppSettings>,
}

impl MainWindow {
    fn new() -> Self {
        MainWindow {
            settings: Arc::new(AppSettings::load()),
        }
    }

    pub fn create<T>(title: T) -> Result<WindowRef, WindowError>
    where
        T: AsRef<str>,
    {
        let geometry = WindowGeometry {
            width: Some(700),
            height: Some(500),
            ..Default::default()
        };

        let main_window = Arc::new(MainWindow::new());

        let win = WindowBuilder::window("miniraw", None)
            .geometry(geometry)
            .title(title.as_ref())
            .icon(IDI_MAINICON)
            .sys_menu_item(
                IDM_DISCARD_FILES,
                "Discard received files",
                main_window.settings.discard_flag(),
            )
            .message_handler(main_window)
            .build()?;

        Ok(win)
    }
}

impl WindowMessageHandler for MainWindow {
    fn handle_message(&self, message: WindowMessage) -> MessageResult {
        match message.msg {
            WM_SYSCOMMAND if message.wparam == IDM_DISCARD_FILES as _ => {
                let flag = !self.settings.discard_flag();
                info!("Discard received files: {}", flag);
                self.settings.set_discard_flag(flag);
                message.window.check_sys_menu_item(IDM_DISCARD_FILES, flag);
                self.settings.store();
                MessageResult::Processed
            }
            WM_CREATE => {
                let edit_style = WS_CHILD
                    | WS_VISIBLE
                    | WS_VSCROLL
                    | WINDOW_STYLE((ES_LEFT | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY) as u32);

                let font = Font::new(14, "Consolas");

                let edit = WindowBuilder::edit_control(message.window)
                    .style(edit_style.0)
                    .extended_style(WS_EX_CLIENTEDGE.0)
                    .font(font)
                    .build()
                    .unwrap();

                logger::WindowLogger::init(edit, LevelFilter::Info);

                info!(
                    ">>> MiniRAW NG {} by Dmitry Pankratov",
                    env!("CARGO_PKG_VERSION")
                );

                info!("Discard received files: {}", self.settings.discard_flag());

                let settings = self.settings.clone();

                std::thread::spawn(move || {
                    if let Err(e) = listener::start_raw_listener(settings) {
                        error!("{}", e);
                    }
                });

                MessageResult::Processed
            }
            WM_SIZE => {
                let gm = WindowGeometry {
                    x: Some(6),
                    y: Some(6),
                    width: Some(((message.lparam as u32) & 0xffff) as i32 - 12),
                    height: Some(((message.lparam as u32) >> 16) as i32 - 12),
                };
                message.window.children()[0].move_window(gm);

                MessageResult::Processed
            }
            WM_DESTROY => {
                MessageLoop::quit();
                MessageResult::Processed
            }
            _ => MessageResult::Ignored,
        }
    }
}

fn main() {
    let _ = MainWindow::create(format!("MiniRAW NG {}", env!("CARGO_PKG_VERSION")));
    MessageLoop::default().run();
}
