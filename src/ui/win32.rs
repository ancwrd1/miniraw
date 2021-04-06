#![allow(dead_code)]

use std::{
    alloc::{self, Layout},
    fmt, mem, ptr,
};

use widestring::{U16String, WideCString};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{errhandlingapi::GetLastError, libloaderapi::GetModuleHandleW, wingdi::*, winuser::*},
};

use crate::ui::window::{
    ControlKind, MessageResult, WindowBuilder, WindowError, WindowGeometry, WindowMessage,
    WindowRef,
};
use winapi::um::winuser::{GetSystemMenu, MF_CHECKED};

pub mod logger;

pub(crate) type HandleType = HWND;

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        let cs = lparam as *const CREATESTRUCTW;
        let mut proxy = (*cs).lpCreateParams as *mut WinProxy;

        (*proxy).hwnd = hwnd;

        SetWindowLongPtrW(hwnd, GWL_USERDATA, proxy as isize);
        (*proxy).window_proc(msg, wparam, lparam)
    } else {
        let data = GetWindowLongPtrW(hwnd, GWL_USERDATA);
        if data != 0 {
            let proxy = data as *mut WinProxy;
            (*proxy).window_proc(msg, wparam, lparam)
        } else {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

pub(crate) struct WinProxy {
    hwnd: HWND,
    owner: Option<WindowRef>,
}

impl fmt::Debug for WinProxy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "hwnd: {:08x}", self.hwnd as u32)
    }
}

impl WinProxy {
    pub(crate) fn new() -> *mut WinProxy {
        let stack_proxy = WinProxy {
            hwnd: ptr::null_mut(),
            owner: None,
        };
        unsafe {
            let proxy = alloc::alloc(Layout::new::<WinProxy>()) as *mut WinProxy;
            ptr::copy(&stack_proxy, proxy, 1);
            proxy
        }
    }

    pub(crate) fn create(
        &mut self,
        builder: &WindowBuilder,
        owner: WindowRef,
    ) -> Result<(), WindowError> {
        unsafe {
            self.owner = Some(owner);

            let hinstance = GetModuleHandleW(ptr::null_mut());
            let style = if builder.style == 0 {
                WS_OVERLAPPEDWINDOW
            } else {
                builder.style
            };

            let class_u16 = match builder.kind {
                ControlKind::Window(ref class) => {
                    let name = WideCString::from_str(class).unwrap();
                    let wnd_class = WNDCLASSW {
                        style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
                        lpfnWndProc: Some(window_proc),
                        hInstance: hinstance,
                        lpszClassName: name.as_ptr(),
                        cbClsExtra: 0,
                        cbWndExtra: 0,
                        hIcon: if let Some(icon) = builder.icon {
                            LoadIconW(
                                GetModuleHandleW(ptr::null_mut()),
                                MAKEINTRESOURCEW(icon as _),
                            )
                        } else {
                            LoadIconW(ptr::null_mut(), IDI_APPLICATION)
                        },
                        hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
                        hbrBackground: COLOR_WINDOW as HBRUSH,
                        lpszMenuName: ptr::null_mut(),
                    };

                    RegisterClassW(&wnd_class);
                    name
                }
                ControlKind::Edit => WideCString::from_str("EDIT").unwrap(),
            };

            let title = WideCString::from_str(&builder.title).unwrap();

            let parent = builder
                .parent
                .as_ref()
                .map(|p| (*p.proxy).hwnd)
                .unwrap_or_else(ptr::null_mut);

            let (x, y, width, height) = builder.geometry.unwrap_or(CW_USEDEFAULT);

            self.hwnd = CreateWindowExW(
                builder.extended_style,
                class_u16.as_ptr(),
                title.as_ptr(),
                style,
                x,
                y,
                width,
                height,
                parent,
                ptr::null_mut(),
                hinstance,
                self as *mut WinProxy as LPVOID,
            );

            if self.hwnd.is_null() {
                let error = GetLastError();
                self.destroy();
                Err(WindowError::CreateError(error as i32))
            } else {
                if let Some(ref font) = builder.font {
                    let face = WideCString::from_str(&font.face).unwrap();

                    let hfont = CreateFontW(
                        font.height as i32,
                        0,
                        0,
                        0,
                        if font.bold { FW_BOLD } else { FW_NORMAL },
                        if font.italics { 1 } else { 0 },
                        0,
                        0,
                        DEFAULT_CHARSET,
                        0,
                        0,
                        DEFAULT_QUALITY,
                        DEFAULT_PITCH,
                        face.as_ptr(),
                    );
                    if !hfont.is_null() {
                        self.send_message(WM_SETFONT, hfont as usize, 1);
                    }
                }

                ShowWindow(self.hwnd, SW_SHOW);
                UpdateWindow(self.hwnd);

                let sys_menu = GetSystemMenu(self.hwnd, FALSE);
                for item in builder.sys_menu_items.iter() {
                    let text_u16 = U16String::from_str(&item.text);
                    let mut info = mem::zeroed::<MENUITEMINFOW>();
                    info.cbSize = mem::size_of::<MENUITEMINFOW>() as _;
                    info.fMask = MIIM_ID | MIIM_STRING;
                    info.wID = item.id;
                    info.dwTypeData = text_u16.as_ptr() as _;
                    info.cch = item.text.len() as _;
                    InsertMenuItemW(sys_menu, GetMenuItemCount(sys_menu) as _, TRUE, &info);
                }
                Ok(())
            }
        }
    }

    pub(crate) fn destroy(&mut self) {
        unsafe {
            if !self.hwnd.is_null() {
                DestroyWindow(self.hwnd);
            }
            alloc::dealloc(self as *mut WinProxy as *mut u8, Layout::new::<WinProxy>());
        }
    }

    pub(crate) fn move_window(&self, geometry: WindowGeometry) {
        unsafe {
            let (x, y, width, height) = geometry.unwrap_or(CW_USEDEFAULT);
            MoveWindow(self.hwnd, x, y, width, height, TRUE);
        }
    }
    pub(crate) fn send_message(&self, msg: u32, wparam: usize, lparam: isize) -> isize {
        unsafe { SendMessageW(self.hwnd, msg, wparam, lparam) }
    }

    pub(crate) fn handle(&self) -> HandleType {
        self.hwnd
    }

    pub(crate) fn check_sys_menu_item(&self, item: u32, flag: bool) {
        unsafe {
            CheckMenuItem(
                GetSystemMenu(self.hwnd, FALSE),
                item,
                if flag { MF_CHECKED } else { MF_UNCHECKED },
            );
        }
    }

    fn window_proc(&mut self, msg: u32, wparam: usize, lparam: isize) -> isize {
        let message = WindowMessage::new(self.owner.as_ref().unwrap().clone(), msg, wparam, lparam);

        let handler = self.owner.as_ref().unwrap().handler.clone();

        match handler.handle_message(message) {
            MessageResult::Processed => 0,
            MessageResult::Ignored => unsafe { DefWindowProcW(self.hwnd, msg, wparam, lparam) },
            MessageResult::Value(value) => value,
        }
    }
}

pub(crate) struct MessageLoopProxy;

impl MessageLoopProxy {
    pub(crate) fn new() -> MessageLoopProxy {
        MessageLoopProxy
    }

    pub(crate) fn run(&self) {
        unsafe {
            let mut message: MSG = mem::zeroed();

            while GetMessageW(&mut message as *mut MSG, ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&message as *const MSG);
                DispatchMessageW(&message as *const MSG);
            }
        }
    }

    pub(crate) fn quit() {
        unsafe {
            PostQuitMessage(0);
        }
    }
}
