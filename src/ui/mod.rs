use win32::MessageLoopProxy;

pub mod win32;
pub mod window;

pub struct MessageLoop {
    proxy: MessageLoopProxy,
}

impl MessageLoop {
    pub fn new() -> MessageLoop {
        MessageLoop {
            proxy: MessageLoopProxy::new(),
        }
    }

    pub fn run(&self) {
        self.proxy.run()
    }

    pub fn quit() {
        MessageLoopProxy::quit()
    }
}
