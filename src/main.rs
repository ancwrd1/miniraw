#![windows_subsystem = "windows"]

use std::{cell::RefCell, error::Error, rc::Rc};

use log::{info, LevelFilter};
use tokio::runtime::Runtime;
use winapi::{
    shared::minwindef::{HIWORD, LOWORD},
    um::winuser::*,
};

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

struct MainWindow {
    runtime: RefCell<Runtime>,
}

impl MainWindow {
    pub fn new<T>(title: T) -> Result<WindowRef, WindowError>
    where
        T: AsRef<str>,
    {
        let geometry = WindowGeometry {
            width: Some(700),
            height: Some(500),
            ..Default::default()
        };

        let main_window = Rc::new(MainWindow {
            runtime: RefCell::new(Runtime::new().unwrap()),
        });

        let win = WindowBuilder::window("miniraw", None)
            .geometry(geometry)
            .title(title.as_ref())
            .message_handler(main_window)
            .build()?;

        Ok(win)
    }
}

impl WindowMessageHandler for MainWindow {
    fn handle_message(&self, message: WindowMessage) -> MessageResult {
        match message.msg {
            WM_CREATE => {
                let edit_style = WS_CHILD
                    | WS_VISIBLE
                    | WS_VSCROLL
                    | ES_LEFT
                    | ES_MULTILINE
                    | ES_AUTOVSCROLL
                    | ES_READONLY;

                let font = Font::new(14, "Consolas");

                let edit = WindowBuilder::edit_control(message.window.clone())
                    .style(edit_style)
                    .extended_style(WS_EX_CLIENTEDGE)
                    .font(font)
                    .build()
                    .unwrap();

                WindowLogger::init(edit.handle(), LevelFilter::Info);

                info!(
                    ">>> MiniRAW NG {} by Dmitry Pankratov",
                    env!("CARGO_PKG_VERSION")
                );

                self.runtime
                    .borrow_mut()
                    .spawn(listener::start_raw_listener());

                MessageResult::Processed
            }
            WM_SIZE => {
                let gm = WindowGeometry {
                    x: Some(6),
                    y: Some(6),
                    width: Some(LOWORD(message.lparam as u32) as i32 - 12),
                    height: Some(HIWORD(message.lparam as u32) as i32 - 12),
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

fn main() -> Result<(), Box<dyn Error>> {
    let _ = MainWindow::new(format!("MiniRAW NG {}", env!("CARGO_PKG_VERSION")))?;
    MessageLoop::new().run();
    Ok(())
}
