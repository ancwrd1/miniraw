#![allow(dead_code)]

use std::{
    alloc::{self, Layout},
    fmt, mem, ptr,
};

use widestring::{U16CString, U16String};
use windows::Win32::{
    Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};

use crate::ui::window::{
    ControlKind, MessageResult, WindowBuilder, WindowError, WindowGeometry, WindowMessage,
    WindowRef,
};

pub mod logger;

pub(crate) type HandleType = HWND;

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        let cs = lparam.0 as *const CREATESTRUCTW;
        let mut proxy = (*cs).lpCreateParams as *mut WinProxy;

        (*proxy).hwnd = hwnd;

        SetWindowLongPtrW(hwnd, GWL_USERDATA, proxy as isize);
        LRESULT((*proxy).window_proc(msg, wparam.0, lparam.0))
    } else {
        let data = GetWindowLongPtrW(hwnd, GWL_USERDATA);
        if data != 0 {
            let proxy = data as *mut WinProxy;
            LRESULT((*proxy).window_proc(msg, wparam.0, lparam.0))
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
        write!(f, "hwnd: {:08x}", self.hwnd.0 as u32)
    }
}

impl WinProxy {
    pub(crate) fn new() -> *mut WinProxy {
        let stack_proxy = WinProxy {
            hwnd: HWND::default(),
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

            let hinstance = GetModuleHandleW(PWSTR::default());
            let style = if builder.style == 0 {
                WS_OVERLAPPEDWINDOW.0
            } else {
                builder.style
            };

            let mut class_u16 = match builder.kind {
                ControlKind::Window(ref class) => {
                    let mut name = U16CString::from_str_unchecked(class);
                    let wnd_class = WNDCLASSW {
                        style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
                        lpfnWndProc: Some(window_proc),
                        hInstance: hinstance,
                        lpszClassName: PWSTR(name.as_mut_ptr()),
                        cbClsExtra: 0,
                        cbWndExtra: 0,
                        hIcon: if let Some(icon) = builder.icon {
                            LoadIconW(GetModuleHandleW(PWSTR::default()), PWSTR(icon as *mut u16))
                        } else {
                            LoadIconW(HINSTANCE::default(), IDI_APPLICATION)
                        },
                        hCursor: LoadCursorW(HINSTANCE::default(), IDC_ARROW),
                        hbrBackground: HBRUSH(COLOR_WINDOW.0 as _),
                        lpszMenuName: PWSTR::default(),
                    };

                    RegisterClassW(&wnd_class);
                    name
                }
                ControlKind::Edit => U16CString::from_str_unchecked("EDIT"),
            };

            let mut title = U16CString::from_str_unchecked(&builder.title);

            let parent = builder
                .parent
                .as_ref()
                .map(|p| (*p.proxy).hwnd)
                .unwrap_or_default();

            let (x, y, width, height) = builder.geometry.unwrap_or(CW_USEDEFAULT);

            self.hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(builder.extended_style),
                PWSTR(class_u16.as_mut_ptr()),
                PWSTR(title.as_mut_ptr()),
                WINDOW_STYLE(style),
                x,
                y,
                width,
                height,
                parent,
                HMENU::default(),
                hinstance,
                self as *mut WinProxy as _,
            );

            if self.hwnd.0 == 0 {
                let error = GetLastError();
                self.destroy();
                Err(WindowError::CreateError(error.0 as i32))
            } else {
                if let Some(ref font) = builder.font {
                    let mut face = U16CString::from_str_unchecked(&font.face);

                    let hfont = CreateFontW(
                        font.height as i32,
                        0,
                        0,
                        0,
                        if font.bold { FW_BOLD } else { FW_NORMAL } as _,
                        if font.italics { 1 } else { 0 },
                        0,
                        0,
                        DEFAULT_CHARSET,
                        FONT_OUTPUT_PRECISION::default(),
                        FONT_CLIP_PRECISION::default(),
                        DEFAULT_QUALITY,
                        FONT_PITCH_AND_FAMILY(DEFAULT_PITCH),
                        PWSTR(face.as_mut_ptr()),
                    );
                    if hfont.0 != 0 {
                        self.send_message(WM_SETFONT, hfont.0 as _, 1);
                    }
                }

                ShowWindow(self.hwnd, SW_SHOW);
                UpdateWindow(self.hwnd);

                let sys_menu = GetSystemMenu(self.hwnd, BOOL(0));
                for item in builder.sys_menu_items.iter() {
                    let mut text_u16 = U16String::from_str(&item.text);
                    let mut info = mem::zeroed::<MENUITEMINFOW>();
                    info.cbSize = mem::size_of::<MENUITEMINFOW>() as _;
                    info.fMask = MIIM_ID | MIIM_STRING;
                    info.wID = item.id;
                    info.dwTypeData = PWSTR(text_u16.as_mut_ptr());
                    info.cch = item.text.len() as _;
                    InsertMenuItemW(sys_menu, GetMenuItemCount(sys_menu) as _, BOOL(1), &info);
                }
                Ok(())
            }
        }
    }

    pub(crate) fn destroy(&mut self) {
        unsafe {
            if self.hwnd.0 != 0 {
                DestroyWindow(self.hwnd);
            }
            alloc::dealloc(self as *mut WinProxy as *mut u8, Layout::new::<WinProxy>());
        }
    }

    pub(crate) fn move_window(&self, geometry: WindowGeometry) {
        unsafe {
            let (x, y, width, height) = geometry.unwrap_or(CW_USEDEFAULT);
            MoveWindow(self.hwnd, x, y, width, height, BOOL(1));
        }
    }
    pub(crate) fn send_message(&self, msg: u32, wparam: usize, lparam: isize) -> isize {
        unsafe { SendMessageW(self.hwnd, msg, WPARAM(wparam), LPARAM(lparam)).0 }
    }

    pub(crate) fn handle(&self) -> HandleType {
        self.hwnd
    }

    pub(crate) fn check_sys_menu_item(&self, item: u32, flag: bool) {
        unsafe {
            CheckMenuItem(
                GetSystemMenu(self.hwnd, BOOL(0)),
                item,
                if flag { MF_CHECKED.0 } else { MF_UNCHECKED.0 },
            );
        }
    }

    fn window_proc(&mut self, msg: u32, wparam: usize, lparam: isize) -> isize {
        let owner = self.owner.as_ref().unwrap().clone();
        let message = WindowMessage::new(owner.clone(), msg, wparam, lparam);

        match owner.handler.handle_message(message) {
            MessageResult::Processed => 0,
            MessageResult::Ignored => unsafe {
                DefWindowProcW(self.hwnd, msg, WPARAM(wparam), LPARAM(lparam)).0
            },
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

            while GetMessageW(&mut message as *mut MSG, HWND::default(), 0, 0).0 > 0 {
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
