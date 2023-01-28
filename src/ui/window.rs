use std::{
    fmt,
    sync::{Arc, RwLock},
};

#[cfg(windows)]
use crate::ui::win32::{HandleType, WinProxy};

pub type WindowRef = Arc<Window>;
pub type WindowHandle = HandleType;

#[derive(Debug)]
pub enum WindowError {
    Win32Error(windows::core::Error),
    InvalidEncoding,
}

impl WindowError {
    pub fn from_win32() -> Self {
        WindowError::Win32Error(windows::core::Error::from_win32())
    }
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WindowError::Win32Error(error) => write!(f, "Windows API error: {error}"),
            WindowError::InvalidEncoding => write!(f, "Invalid text encoding (expected UTF-16)"),
        }
    }
}

impl From<windows::core::Error> for WindowError {
    fn from(err: windows::core::Error) -> Self {
        WindowError::Win32Error(err)
    }
}

impl std::error::Error for WindowError {}

#[derive(Debug, Clone)]
pub struct Font {
    pub height: u32,
    pub bold: bool,
    pub italics: bool,
    pub face: String,
}

impl Font {
    pub fn new<S>(height: u32, face: S) -> Font
    where
        S: AsRef<str>,
    {
        Font {
            height,
            bold: false,
            italics: false,
            face: face.as_ref().to_owned(),
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct WindowGeometry {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

impl WindowGeometry {
    pub fn zero() -> WindowGeometry {
        WindowGeometry {
            x: Some(0),
            y: Some(0),
            width: Some(0),
            height: Some(0),
        }
    }

    pub fn unwrap_or(&self, default: i32) -> (i32, i32, i32, i32) {
        (
            self.x.unwrap_or(default),
            self.y.unwrap_or(default),
            self.width.unwrap_or(default),
            self.height.unwrap_or(default),
        )
    }
}

pub(crate) enum ControlKind {
    Window(String),
    Edit,
}

pub(crate) struct MenuItem {
    pub(crate) id: u32,
    pub(crate) text: String,
    pub(crate) checked: bool,
}

struct DummyMessageHandler;
impl WindowMessageHandler for DummyMessageHandler {}

pub struct WindowBuilder {
    pub(crate) kind: ControlKind,
    pub(crate) title: String,
    pub(crate) geometry: WindowGeometry,
    pub(crate) style: u32,
    pub(crate) extended_style: u32,
    pub(crate) parent: Option<WindowRef>,
    pub(crate) handler: Arc<dyn WindowMessageHandler + Send + Sync + 'static>,
    pub(crate) font: Option<Font>,
    pub(crate) icon: Option<u32>,
    pub(crate) sys_menu_items: Vec<MenuItem>,
}

impl WindowBuilder {
    pub fn window<S>(class: S, parent: Option<WindowRef>) -> WindowBuilder
    where
        S: AsRef<str>,
    {
        WindowBuilder {
            kind: ControlKind::Window(class.as_ref().to_owned()),
            title: String::new(),
            geometry: WindowGeometry::default(),
            style: 0,
            extended_style: 0,
            parent,
            handler: Arc::new(DummyMessageHandler),
            font: None,
            icon: None,
            sys_menu_items: Vec::new(),
        }
    }

    pub fn edit_control(parent: WindowRef) -> WindowBuilder {
        WindowBuilder {
            kind: ControlKind::Edit,
            title: String::new(),
            geometry: WindowGeometry::zero(),
            style: 0,
            extended_style: 0,
            parent: Some(parent),
            handler: Arc::new(DummyMessageHandler),
            font: None,
            icon: None,
            sys_menu_items: Vec::new(),
        }
    }

    pub fn title<T>(mut self, title: T) -> Self
    where
        T: AsRef<str>,
    {
        self.title = title.as_ref().to_owned();
        self
    }

    pub fn geometry(mut self, geometry: WindowGeometry) -> Self {
        self.geometry = geometry;
        self
    }

    pub fn style(mut self, style: u32) -> Self {
        self.style = style;
        self
    }

    pub fn extended_style(mut self, style: u32) -> Self {
        self.extended_style = style;
        self
    }

    pub fn message_handler(
        mut self,
        handler: Arc<dyn WindowMessageHandler + Send + Sync + 'static>,
    ) -> Self {
        self.handler = handler;
        self
    }

    pub fn font(mut self, font: Font) -> Self {
        self.font = Some(font);
        self
    }

    pub fn icon(mut self, icon: u32) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn sys_menu_item<T>(mut self, id: u32, text: T, checked: bool) -> Self
    where
        T: AsRef<str>,
    {
        self.sys_menu_items.push(MenuItem {
            id,
            text: text.as_ref().to_owned(),
            checked,
        });
        self
    }

    pub fn build(mut self) -> Result<WindowRef, WindowError> {
        let window = Arc::new(Window {
            proxy: WinProxy::new(),
            children: Default::default(),
            handler: self.handler.clone(),
        });

        if let Some(parent) = self.parent.as_mut() {
            parent.add_child(window.clone());
        }

        window.proxy().create(&self, window.clone())?;

        Ok(window)
    }
}

#[derive(Debug, Clone)]
pub struct WindowMessage {
    pub window: WindowRef,
    pub msg: u32,
    pub wparam: usize,
    pub lparam: isize,
}

impl WindowMessage {
    pub fn new(window: WindowRef, msg: u32, wparam: usize, lparam: isize) -> WindowMessage {
        WindowMessage {
            window,
            msg,
            wparam,
            lparam,
        }
    }
}

pub enum MessageResult {
    Processed,
    Ignored,
    Value(isize),
}

pub struct Window {
    pub(crate) proxy: *mut WinProxy,
    pub(crate) children: RwLock<Vec<WindowRef>>,
    pub(crate) handler: Arc<dyn WindowMessageHandler + Send + Sync + 'static>,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl fmt::Debug for Window {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.proxy())
    }
}
impl Window {
    #[allow(clippy::mut_from_ref)]
    fn proxy(&self) -> &mut WinProxy {
        unsafe { &mut *self.proxy }
    }

    pub fn children(&self) -> Vec<WindowRef> {
        self.children.read().unwrap().iter().cloned().collect()
    }

    pub fn send_message(&self, message: WindowMessage) -> MessageResult {
        let result = self
            .proxy()
            .send_message(message.msg, message.wparam, message.lparam);
        MessageResult::Value(result)
    }

    pub fn move_window(&self, geometry: WindowGeometry) {
        self.proxy().move_window(geometry)
    }

    pub fn handle(&self) -> WindowHandle {
        self.proxy().handle()
    }

    pub fn add_child(&self, child: WindowRef) {
        self.children.write().unwrap().push(child)
    }

    pub fn check_sys_menu_item(&self, item: u32, flag: bool) {
        self.proxy().check_sys_menu_item(item, flag)
    }

    pub fn get_text(&self) -> Result<String, WindowError> {
        self.proxy().get_text()
    }

    pub fn set_text(&self, text: &str) -> Result<(), WindowError> {
        self.proxy().set_text(text)
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.proxy().destroy();
    }
}

pub trait WindowMessageHandler {
    fn handle_message(&self, _message: WindowMessage) -> MessageResult {
        MessageResult::Ignored
    }
}
