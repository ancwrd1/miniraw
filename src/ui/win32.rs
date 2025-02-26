use std::{fmt, mem};

use windows::{
    Win32::{
        Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
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

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if msg == WM_CREATE {
            let cs = lparam.0 as *const CREATESTRUCTW;
            let proxy = (*cs).lpCreateParams as *mut WinProxy;

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

            let hinstance = GetModuleHandleW(PCWSTR::null())?.into();
            let style = if builder.style == 0 {
                WS_OVERLAPPEDWINDOW.0
            } else {
                builder.style
            };

            let class_u16 = match builder.kind {
                ControlKind::Window(ref class) => {
                    let name = utf16z!(class);
                    let wnd_class = WNDCLASSW {
                        style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
                        lpfnWndProc: Some(window_proc),
                        hInstance: hinstance,
                        lpszClassName: PCWSTR(name.as_ptr()),
                        cbClsExtra: 0,
                        cbWndExtra: 0,
                        hIcon: if let Some(icon) = builder.icon {
                            LoadIconW(
                                Some(GetModuleHandleW(PCWSTR::null())?.into()),
                                PCWSTR(icon as *const u16),
                            )?
                        } else {
                            LoadIconW(None, IDI_APPLICATION)?
                        },
                        hCursor: LoadCursorW(None, IDC_ARROW)?,
                        hbrBackground: HBRUSH(COLOR_WINDOW.0 as _),
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
                WINDOW_EX_STYLE(builder.extended_style),
                PCWSTR(class_u16.as_ptr()),
                PCWSTR(title.as_ptr()),
                WINDOW_STYLE(style),
                x,
                y,
                width,
                height,
                Some(parent),
                None,
                Some(hinstance),
                Some(self as *mut WinProxy as _),
            )?;
            if let Some(ref font) = builder.font {
                let face = utf16z!(font.face);

                let hfont = CreateFontW(
                    font.height as i32,
                    0,
                    0,
                    0,
                    if font.bold { FW_BOLD.0 } else { FW_NORMAL.0 } as _,
                    font.italics as u32,
                    0,
                    0,
                    DEFAULT_CHARSET,
                    FONT_OUTPUT_PRECISION::default(),
                    FONT_CLIP_PRECISION::default(),
                    DEFAULT_QUALITY,
                    DEFAULT_PITCH.0 as _,
                    PCWSTR(face.as_ptr()),
                );
                if !hfont.is_invalid() {
                    self.send_message(WM_SETFONT, hfont.0 as _, 1);
                }
            }

            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = UpdateWindow(self.hwnd);

            let sys_menu = GetSystemMenu(self.hwnd, false);
            for item in builder.sys_menu_items.iter() {
                let mut text_u16 = utf16z!(item.text);
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
                InsertMenuItemW(sys_menu, GetMenuItemCount(Some(sys_menu)) as _, true, &info)?;
            }
            Ok(())
        }
    }

    pub(crate) fn destroy(&mut self) {
        unsafe {
            if !self.hwnd.is_invalid() {
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
        unsafe { SendMessageW(self.hwnd, msg, Some(WPARAM(wparam)), Some(LPARAM(lparam))) }
    }

    pub(crate) fn handle(&self) -> HandleType {
        self.hwnd
    }

    pub(crate) fn check_sys_menu_item(&self, item: u32, flag: bool) {
        unsafe {
            CheckMenuItem(
                GetSystemMenu(self.hwnd, false),
                item,
                if flag { MF_CHECKED.0 } else { MF_UNCHECKED.0 },
            );
        }
    }

    pub fn get_text(&self) -> Result<String, WindowError> {
        let mut lresult = self.send_message(WM_GETTEXTLENGTH, 0, 0);

        if lresult.0 < 0 {
            return Err(WindowError::from_win32());
        }

        let mut buffer = vec![0u16; lresult.0 as usize + 1];

        lresult = self.send_message(WM_GETTEXT, buffer.len() as _, buffer.as_mut_ptr() as _);

        if lresult.0 < 0 {
            return Err(WindowError::from_win32());
        }

        String::from_utf16(&buffer[0..lresult.0 as usize]).map_err(|_| WindowError::InvalidEncoding)
    }

    pub fn set_text(&self, text: &str) -> Result<(), WindowError> {
        let msg = utf16z!(text);
        let result = self.send_message(WM_SETTEXT, 0, msg.as_ptr() as _).0 != 0;

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
                MessageResult::Ignored => {
                    DefWindowProcW(self.hwnd, msg, WPARAM(wparam), LPARAM(lparam)).0
                }
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
