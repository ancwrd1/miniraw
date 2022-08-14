#![windows_subsystem = "windows"]

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use log::{error, info, LevelFilter};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            RegCloseKey, RegCreateKeyW, RegOpenKeyW, RegQueryValueExW, RegSetKeyValueW, HKEY,
            HKEY_CURRENT_USER, REG_DWORD,
        },
        UI::WindowsAndMessaging::*,
    },
};

use crate::ui::{
    window::{
        Font, MessageResult, WindowBuilder, WindowError, WindowGeometry, WindowMessage,
        WindowMessageHandler, WindowRef,
    },
    MessageLoop,
};

pub mod listener;
pub mod logger;
pub mod ui;
pub mod util;

const IDI_MAINICON: u32 = 1000;
const IDM_DISCARD_FILES: u32 = 1001;
const REG_KEY_NAME: &str = "Software\\MiniRAW NG";
const REG_VALUE_NAME: &str = "discard";

struct MainWindow {
    discard_flag: Arc<AtomicBool>,
}

impl MainWindow {
    fn new() -> Self {
        let window = MainWindow {
            discard_flag: Arc::new(AtomicBool::new(false)),
        };
        window.load_discard_flag();
        window
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
                main_window.discard_flag.load(Ordering::SeqCst),
            )
            .message_handler(main_window)
            .build()?;

        Ok(win)
    }

    fn load_discard_flag(&self) {
        unsafe {
            let mut hkey = HKEY::default();
            let key_name = utf16z!(REG_KEY_NAME);
            let value_name = utf16z!(REG_VALUE_NAME);
            if RegOpenKeyW(HKEY_CURRENT_USER, PCWSTR(key_name.as_ptr()), &mut hkey) == ERROR_SUCCESS
            {
                let mut data = [0u8; 4];
                let mut size = data.len() as u32;
                if RegQueryValueExW(
                    hkey,
                    PCWSTR(value_name.as_ptr()),
                    &mut 0,
                    None,
                    data.as_mut_ptr(),
                    Some(&mut size),
                ) == ERROR_SUCCESS
                {
                    self.discard_flag
                        .store(u32::from_ne_bytes(data) != 0, Ordering::SeqCst);
                }
                RegCloseKey(hkey);
            }
        }
    }

    fn store_discard_flag(&self) {
        unsafe {
            let mut hkey = HKEY::default();
            let key_name = utf16z!(REG_KEY_NAME);
            let value_name = utf16z!(REG_VALUE_NAME);
            let rc = RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(key_name.as_ptr()), &mut hkey);
            if rc == ERROR_SUCCESS {
                let mut data = (self.discard_flag.load(Ordering::SeqCst) as u32).to_ne_bytes();
                RegSetKeyValueW(
                    hkey,
                    PCWSTR::null(),
                    PCWSTR(value_name.as_ptr()),
                    REG_DWORD.0,
                    Some(data.as_mut()),
                );
                RegCloseKey(hkey);
            }
        }
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
                self.store_discard_flag();
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

                info!(
                    "Discard received files: {}",
                    self.discard_flag.load(Ordering::SeqCst)
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
    MessageLoop::default().run();
}
