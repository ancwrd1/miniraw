use std::{fmt, mem};

use windows::{
    Win32::{
        CLIP_DEFAULT_PRECIS, COLOR_WINDOW, CREATESTRUCTW, CS_HREDRAW, CS_OWNDC, CS_VREDRAW,
        CW_USEDEFAULT, CheckMenuItem, CreateFontW, CreateWindowExW, DEFAULT_CHARSET, DEFAULT_PITCH,
        DEFAULT_QUALITY, DefWindowProcW, DestroyWindow, DispatchMessageW, FW_BOLD, FW_NORMAL,
        GWLP_USERDATA, GetMenuItemCount, GetMessageW, GetModuleHandleW, GetSystemMenu,
        GetWindowLongPtrW, HFONT, HICON, HINSTANCE, HWND, IDC_ARROW, IDI_APPLICATION,
        InsertMenuItemW, LPARAM, LRESULT, LoadCursorW, LoadIconW, MENUITEMINFOW, MF_CHECKED,
        MF_UNCHECKED, MFS_CHECKED, MFS_UNCHECKED, MIIM_ID, MIIM_STATE, MIIM_STRING, MSG,
        MoveWindow, OUT_DEFAULT_PRECIS, PostQuitMessage, RegisterClassW, SW_SHOW, SendMessageW,
        SetWindowLongPtrW, ShowWindow, TranslateMessage, UpdateWindow, WM_CREATE, WM_GETTEXT,
        WM_GETTEXTLENGTH, WM_SETFONT, WM_SETTEXT, WNDCLASSW, WPARAM, WS_OVERLAPPEDWINDOW,
    },
    core::{PCWSTR, PWSTR},
};

use crate::{
    ui::window::{
        ControlKind, MessageResult, WindowBuilder, WindowError, WindowGeometry, WindowMessage,
        WindowRef,
    },
    utf16z,
};

pub(crate) type HandleType = HWND;

/// The Win32 bindings return raw handles, so null checks replace the previous `Result` returns.
trait HandleExt: Sized {
    fn is_null_ptr(&self) -> bool;

    fn ok(self) -> Result<Self, WindowError> {
        if self.is_null_ptr() {
            Err(WindowError::from_win32())
        } else {
            Ok(self)
        }
    }
}

macro_rules! impl_handle_ext {
    ($($ty:ty),*) => {
        $(impl HandleExt for $ty {
            fn is_null_ptr(&self) -> bool {
                self.is_null()
            }
        })*
    };
}

// `HCURSOR` is an alias of `HICON`, so it is covered by the latter.
impl_handle_ext!(HWND, HINSTANCE, HICON, HFONT);

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if msg == WM_CREATE as u32 {
            let cs = lparam as *const CREATESTRUCTW;
            let proxy = (*cs).lpCreateParams as *mut WinProxy;

            (*proxy).hwnd = hwnd;

            SetWindowLongPtrW(hwnd, GWLP_USERDATA, proxy as isize);
            (*proxy).window_proc(msg, wparam, lparam)
        } else {
            let data = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if data != 0 {
                let proxy = data as *mut WinProxy;
                (*proxy).window_proc(msg, wparam, lparam)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
    }
}

pub(crate) struct WinProxy {
    hwnd: HWND,
    owner: Option<WindowRef>,
}

impl fmt::Debug for WinProxy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "hwnd: {:08x}", self.hwnd as usize as u32)
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

            let hinstance = GetModuleHandleW(PCWSTR::null()).ok()?;
            let style = if builder.style == 0 {
                WS_OVERLAPPEDWINDOW as u32
            } else {
                builder.style
            };

            let class_u16 = match builder.kind {
                ControlKind::Window(ref class) => {
                    let name = utf16z!(class);
                    let wnd_class = WNDCLASSW {
                        style: (CS_OWNDC | CS_HREDRAW | CS_VREDRAW) as u32,
                        lpfnWndProc: Some(window_proc),
                        hInstance: hinstance,
                        lpszClassName: PCWSTR(name.as_ptr()),
                        cbClsExtra: 0,
                        cbWndExtra: 0,
                        hIcon: if let Some(icon) = builder.icon {
                            LoadIconW(Some(hinstance), PCWSTR(icon as *const u16)).ok()?
                        } else {
                            LoadIconW(None, PCWSTR(IDI_APPLICATION.0 as *const u16)).ok()?
                        },
                        hCursor: LoadCursorW(None, PCWSTR(IDC_ARROW.0 as *const u16)).ok()?,
                        hbrBackground: COLOR_WINDOW as isize as _,
                        lpszMenuName: PCWSTR::null(),
                    };

                    RegisterClassW(&wnd_class);
                    name
                }
                ControlKind::Edit => utf16z!("EDIT"),
            };

            let title = utf16z!(builder.title);

            let parent = builder
                .parent
                .as_ref()
                .map(|p| (*p.proxy).hwnd)
                .unwrap_or_default();

            let (x, y, width, height) = builder.geometry.unwrap_or(CW_USEDEFAULT);

            self.hwnd = CreateWindowExW(
                builder.extended_style,
                PCWSTR(class_u16.as_ptr()),
                PCWSTR(title.as_ptr()),
                style,
                x,
                y,
                width,
                height,
                Some(parent),
                None,
                Some(hinstance),
                Some(self as *mut WinProxy as _),
            )
            .ok()?;
            if let Some(ref font) = builder.font {
                let face = utf16z!(font.face);

                let hfont = CreateFontW(
                    font.height as i32,
                    0,
                    0,
                    0,
                    if font.bold { FW_BOLD } else { FW_NORMAL },
                    font.italics as u32,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    OUT_DEFAULT_PRECIS as u32,
                    CLIP_DEFAULT_PRECIS as u32,
                    DEFAULT_QUALITY as u32,
                    DEFAULT_PITCH as u32,
                    PCWSTR(face.as_ptr()),
                );
                if !hfont.is_null() {
                    self.send_message(WM_SETFONT as u32, hfont as _, 1);
                }
            }

            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = UpdateWindow(self.hwnd);

            let sys_menu = GetSystemMenu(self.hwnd, false);
            for item in builder.sys_menu_items.iter() {
                let mut text_u16 = utf16z!(item.text);
                let mut info = mem::zeroed::<MENUITEMINFOW>();
                info.cbSize = mem::size_of::<MENUITEMINFOW>() as _;
                info.fMask = (MIIM_ID | MIIM_STRING | MIIM_STATE) as u32;
                info.wID = item.id;
                info.fState = if item.checked {
                    MFS_CHECKED as u32
                } else {
                    MFS_UNCHECKED as u32
                };
                info.dwTypeData = PWSTR(text_u16.as_mut_ptr());
                info.cch = item.text.len() as _;
                InsertMenuItemW(sys_menu, GetMenuItemCount(Some(sys_menu)) as _, true, &info)
                    .ok()?;
            }
            Ok(())
        }
    }

    pub(crate) fn destroy(&mut self) {
        unsafe {
            if !self.hwnd.is_null() {
                let _ = DestroyWindow(self.hwnd);
            }
            let _ = Box::from_raw(self);
        }
    }

    pub(crate) fn move_window(&self, geometry: WindowGeometry) {
        unsafe {
            let (x, y, width, height) = geometry.unwrap_or(CW_USEDEFAULT);
            let _ = MoveWindow(self.hwnd, x, y, width, height, true);
        }
    }
    pub(crate) fn send_message(&self, msg: u32, wparam: usize, lparam: isize) -> LRESULT {
        unsafe { SendMessageW(self.hwnd, msg, wparam, lparam) }
    }

    pub(crate) fn handle(&self) -> HandleType {
        self.hwnd
    }

    pub(crate) fn check_sys_menu_item(&self, item: u32, flag: bool) {
        unsafe {
            CheckMenuItem(
                GetSystemMenu(self.hwnd, false),
                item,
                if flag { MF_CHECKED } else { MF_UNCHECKED } as u32,
            );
        }
    }

    pub fn get_text(&self) -> Result<String, WindowError> {
        let mut lresult = self.send_message(WM_GETTEXTLENGTH as u32, 0, 0);

        if lresult < 0 {
            return Err(WindowError::from_win32());
        }

        let mut buffer = vec![0u16; lresult as usize + 1];

        lresult = self.send_message(
            WM_GETTEXT as u32,
            buffer.len() as _,
            buffer.as_mut_ptr() as _,
        );

        if lresult < 0 {
            return Err(WindowError::from_win32());
        }

        String::from_utf16(&buffer[0..lresult as usize]).map_err(|_| WindowError::InvalidEncoding)
    }

    pub fn set_text(&self, text: &str) -> Result<(), WindowError> {
        let msg = utf16z!(text);
        let result = self.send_message(WM_SETTEXT as u32, 0, msg.as_ptr() as _) != 0;

        if result {
            Ok(())
        } else {
            Err(WindowError::from_win32())
        }
    }

    unsafe fn window_proc(&mut self, msg: u32, wparam: usize, lparam: isize) -> isize {
        unsafe {
            let owner = self.owner.as_ref().unwrap().clone();
            let message = WindowMessage::new(owner.clone(), msg, wparam, lparam);

            match owner.handler.handle_message(message) {
                MessageResult::Processed => 0,
                MessageResult::Ignored => DefWindowProcW(self.hwnd, msg, wparam, lparam),
                MessageResult::Value(value) => value,
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct MessageLoopProxy;

impl MessageLoopProxy {
    pub(crate) fn run(&self) {
        unsafe {
            let mut message: MSG = mem::zeroed();

            while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    pub(crate) fn quit() {
        unsafe {
            PostQuitMessage(0);
        }
    }
}
