use win32::MessageLoopProxy;

pub mod win32;
pub mod window;

#[derive(Default)]
pub struct MessageLoop {
    proxy: MessageLoopProxy,
}

impl MessageLoop {
    pub fn run(&self) {
        self.proxy.run()
    }

    pub fn quit() {
        MessageLoopProxy::quit()
    }
}
