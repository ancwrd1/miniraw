use std::{fmt, mem};

use widestring::{U16CStr, U16CString};
use windows::Win32::{
    Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};

use crate::ui::window::{
    ControlKind, MessageResult, WindowBuilder, WindowError, WindowGeometry, WindowMessage,
    WindowRef,
};

pub(crate) type HandleType = HWND;

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
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
        Box::into_raw(Box::new(WinProxy {
            hwnd: HWND::default(),
            owner: None,
        }))
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
                WS_OVERLAPPEDWINDOW
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
                        hbrBackground: COLOR_WINDOW as _,
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
                builder.extended_style,
                PWSTR(class_u16.as_mut_ptr()),
                PWSTR(title.as_mut_ptr()),
                style,
                x,
                y,
                width,
                height,
                parent,
                HMENU::default(),
                hinstance,
                self as *mut WinProxy as _,
            );

            if self.hwnd == 0 {
                let err = WindowError::Win32Error(windows::core::Error::from_win32());
                self.destroy();
                Err(err)
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
                        DEFAULT_PITCH,
                        PWSTR(face.as_mut_ptr()),
                    );
                    if hfont != 0 {
                        self.send_message(WM_SETFONT, hfont as _, 1);
                    }
                }

                ShowWindow(self.hwnd, SW_SHOW);
                UpdateWindow(self.hwnd);

                let sys_menu = GetSystemMenu(self.hwnd, BOOL(0));
                for item in builder.sys_menu_items.iter() {
                    let mut text_u16 = U16CString::from_str_unchecked(&item.text);
                    let mut info = mem::zeroed::<MENUITEMINFOW>();
                    info.cbSize = mem::size_of::<MENUITEMINFOW>() as _;
                    info.fMask = MIIM_ID | MIIM_STRING | MIIM_STATE;
                    info.wID = item.id;
                    info.fState = if item.checked {
                        MFS_CHECKED
                    } else {
                        MFS_UNCHECKED
                    };
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
            if self.hwnd != 0 {
                DestroyWindow(self.hwnd);
            }
            Box::from_raw(self);
        }
    }

    pub(crate) fn move_window(&self, geometry: WindowGeometry) {
        unsafe {
            let (x, y, width, height) = geometry.unwrap_or(CW_USEDEFAULT);
            MoveWindow(self.hwnd, x, y, width, height, BOOL(1));
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
                GetSystemMenu(self.hwnd, BOOL(0)),
                item,
                if flag { MF_CHECKED } else { MF_UNCHECKED },
            );
        }
    }

    pub fn get_text(&self) -> Result<String, WindowError> {
        unsafe {
            let mut text_len = self.send_message(WM_GETTEXTLENGTH, 0, 0);

            if text_len < 0 {
                return Err(WindowError::from_win32());
            }

            let mut buffer = vec![0u16; text_len as usize + 1];

            text_len = self.send_message(WM_GETTEXT, buffer.len() as _, buffer.as_mut_ptr() as _);

            if text_len < 0 {
                return Err(WindowError::from_win32());
            }

            Ok(U16CStr::from_slice_unchecked(&buffer[0..text_len as usize + 1]).to_string_lossy())
        }
    }

    pub fn set_text(&self, text: &str) -> Result<(), WindowError> {
        unsafe {
            let msg = U16CString::from_str_unchecked(text);
            let result = self.send_message(WM_SETTEXT, 0, msg.as_ptr() as _) != 0;

            if result {
                Ok(())
            } else {
                Err(WindowError::from_win32())
            }
        }
    }

    unsafe fn window_proc(&mut self, msg: u32, wparam: usize, lparam: isize) -> isize {
        let owner = self.owner.as_ref().unwrap().clone();
        let message = WindowMessage::new(owner.clone(), msg, wparam, lparam);

        match owner.handler.handle_message(message) {
            MessageResult::Processed => 0,
            MessageResult::Ignored => {
                DefWindowProcW(self.hwnd, msg, wparam, lparam)
            }
            MessageResult::Value(value) => value,
        }
    }
}

#[derive(Default)]
pub(crate) struct MessageLoopProxy;

impl MessageLoopProxy {
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
