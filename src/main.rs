#![windows_subsystem = "windows"]

use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use log::{error, info, LevelFilter};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::ui::{
    win32::logger::WindowLogger,
    window::{
        Font, MessageResult, WindowBuilder, WindowError, WindowGeometry, WindowMessage,
        WindowMessageHandler, WindowRef,
    },
    MessageLoop,
};

mod listener;
mod ui;

const IDI_MAINICON: u32 = 1000;
const IDM_DISCARD_FILES: u32 = 1001;

struct MainWindow {
    discard_flag: Arc<AtomicBool>,
}

impl MainWindow {
    pub fn create<T>(title: T) -> Result<WindowRef, WindowError>
    where
        T: AsRef<str>,
    {
        let geometry = WindowGeometry {
            width: Some(700),
            height: Some(500),
            ..Default::default()
        };

        let main_window = Rc::new(MainWindow {
            discard_flag: Arc::new(AtomicBool::new(false)),
        });

        let win = WindowBuilder::window("miniraw", None)
            .geometry(geometry)
            .title(title.as_ref())
            .icon(IDI_MAINICON)
            .sys_menu_item(IDM_DISCARD_FILES, "Discard received files")
            .message_handler(main_window)
            .build()?;

        Ok(win)
    }
}

impl WindowMessageHandler for MainWindow {
    fn handle_message(&self, message: WindowMessage) -> MessageResult {
        match message.msg {
            WM_SYSCOMMAND if message.wparam == IDM_DISCARD_FILES as _ => {
                let flag = !self.discard_flag.load(Ordering::SeqCst);
                info!("Discard received files: {}", flag);
                self.discard_flag.store(flag, Ordering::SeqCst);
                message.window.check_sys_menu_item(IDM_DISCARD_FILES, flag);
                MessageResult::Processed
            }
            WM_CREATE => {
                let edit_style = WS_CHILD
                    | WS_VISIBLE
                    | WS_VSCROLL
                    | WINDOW_STYLE((ES_LEFT | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY) as _);

                let font = Font::new(14, "Consolas");

                let edit = WindowBuilder::edit_control(message.window)
                    .style(edit_style.0)
                    .extended_style(WS_EX_CLIENTEDGE.0)
                    .font(font)
                    .build()
                    .unwrap();

                WindowLogger::init(edit.handle(), LevelFilter::Info);

                info!(
                    ">>> MiniRAW NG {} by Dmitry Pankratov",
                    env!("CARGO_PKG_VERSION")
                );

                let flag = self.discard_flag.clone();

                std::thread::spawn(|| {
                    if let Err(e) = listener::start_raw_listener(flag) {
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
    MessageLoop::new().run();
}
