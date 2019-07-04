#![allow(dead_code)]

use std::{cell::RefCell, fmt, rc::Rc};

use crate::ui::win32::{HandleType, WinProxy};

pub type WindowRef = Rc<Window>;
pub type WindowHandle = HandleType;

#[derive(Debug)]
pub enum WindowError {
    CreateError(i32),
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WindowError::CreateError(code) => write!(f, "Create window error: {}", code),
        }
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

struct DummyMessageHandler;
impl WindowMessageHandler for DummyMessageHandler {}

pub struct WindowBuilder {
    pub(crate) kind: ControlKind,
    pub(crate) title: String,
    pub(crate) geometry: WindowGeometry,
    pub(crate) style: u32,
    pub(crate) extended_style: u32,
    pub(crate) parent: Option<WindowRef>,
    pub(crate) handler: Rc<dyn WindowMessageHandler>,
    pub(crate) font: Option<Font>,
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
            handler: Rc::new(DummyMessageHandler),
            font: None,
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
            handler: Rc::new(DummyMessageHandler),
            font: None,
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

    pub fn message_handler(mut self, handler: Rc<dyn WindowMessageHandler>) -> Self {
        self.handler = handler;
        self
    }

    pub fn font(mut self, font: Font) -> Self {
        self.font = Some(font);
        self
    }

    pub fn build(mut self) -> Result<WindowRef, WindowError> {
        let window = Rc::new(Window {
            proxy: WinProxy::new(),
            children: RefCell::new(Vec::new()),
            handler: self.handler.clone(),
        });

        if let Some(parent) = self.parent.as_mut() {
            parent.add_child(window.clone());
        }

        unsafe { (*window.proxy).create(&self, window.clone())? };

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
    pub(crate) children: RefCell<Vec<WindowRef>>,
    pub(crate) handler: Rc<dyn WindowMessageHandler>,
}

impl fmt::Debug for Window {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe { write!(f, "{:?}", *self.proxy) }
    }
}
impl Window {
    pub fn children(&self) -> Vec<WindowRef> {
        self.children.borrow().iter().map(|w| w.clone()).collect()
    }

    pub fn send_message(&self, message: WindowMessage) -> MessageResult {
        let result =
            unsafe { (*self.proxy).send_message(message.msg, message.wparam, message.lparam) };
        MessageResult::Value(result)
    }

    pub fn move_window(&self, geometry: WindowGeometry) {
        unsafe { (*self.proxy).move_window(geometry) }
    }

    pub fn handle(&self) -> WindowHandle {
        unsafe { (*self.proxy).handle() }
    }

    pub fn add_child(&self, child: WindowRef) {
        self.children.borrow_mut().push(child)
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            (*self.proxy).destroy();
        };
    }
}

pub trait WindowMessageHandler {
    fn handle_message(&self, _message: WindowMessage) -> MessageResult {
        MessageResult::Ignored
    }
}
