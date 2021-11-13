#![windows_subsystem = "windows"]

use std::{
    mem, ptr,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use log::{error, info, LevelFilter};
use windows::Win32::{
    Foundation::{ERROR_SUCCESS, PWSTR},
    System::Registry::{
        RegCloseKey, RegCreateKeyW, RegOpenKeyW, RegQueryValueExW, RegSetKeyValueW, HKEY,
        HKEY_CURRENT_USER, REG_DWORD,
    },
    UI::WindowsAndMessaging::*,
};

use crate::ui::{
    win32::logger::WindowLogger,
    window::{
        Font, MessageResult, WindowBuilder, WindowError, WindowGeometry, WindowMessage,
        WindowMessageHandler, WindowRef,
    },
    MessageLoop,
};

pub mod listener;
pub mod ui;

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

        let main_window = Rc::new(MainWindow::new());

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
            if RegOpenKeyW(HKEY_CURRENT_USER, REG_KEY_NAME, &mut hkey).0 == ERROR_SUCCESS.0 as i32 {
                let mut data = 0u32;
                let mut size = mem::size_of::<u32>() as u32;
                if RegQueryValueExW(
                    hkey,
                    REG_VALUE_NAME,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut data as *mut u32 as _,
                    &mut size,
                )
                .0 == ERROR_SUCCESS.0 as i32
                {
                    self.discard_flag.store(data != 0, Ordering::SeqCst);
                }
                RegCloseKey(hkey);
            }
        }
    }

    fn store_discard_flag(&self) {
        unsafe {
            let mut hkey = HKEY::default();
            if RegCreateKeyW(HKEY_CURRENT_USER, REG_KEY_NAME, &mut hkey).0 == ERROR_SUCCESS.0 as i32
            {
                let mut data = self.discard_flag.load(Ordering::SeqCst) as u32;
                RegSetKeyValueW(
                    hkey,
                    PWSTR::default(),
                    REG_VALUE_NAME,
                    REG_DWORD.0,
                    &mut data as *mut u32 as _,
                    mem::size_of::<u32>() as u32,
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
                    | WINDOW_STYLE((ES_LEFT | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY) as _);

                let font = Font::new(14, "Consolas");

                let edit = WindowBuilder::edit_control(message.window)
                    .style(edit_style.0)
                    .extended_style(WS_EX_CLIENTEDGE.0)
                    .font(font)
                    .build()
                    .unwrap();

                WindowLogger::init(edit, LevelFilter::Info);

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
