use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    time,
};

use futures::{future::IntoFuture, stream::Stream, try_ready, Async, Future, Poll};
use log::{info, warn};
use tokio::{io::AsyncRead, net::TcpListener};

struct FileStream {
    inner: Box<AsyncRead + Send>,
    buffer: Vec<u8>,
}

impl FileStream {
    const CHUNK_SIZE: usize = 32768;

    fn new(reader: Box<AsyncRead + Send>) -> FileStream {
        FileStream {
            inner: reader,
            buffer: vec![0; FileStream::CHUNK_SIZE],
        }
    }
}

impl Stream for FileStream {
    type Item = Vec<u8>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let size = try_ready!(self.inner.poll_read(&mut self.buffer));
        if size > 0 {
            Ok(Async::Ready(Some(self.buffer[0..size].into())))
        } else {
            Ok(Async::Ready(None))
        }
    }
}

pub fn start_raw_listener() -> impl Future<Item = (), Error = ()> {
    let addr = "0.0.0.0:9100".parse().unwrap();
    TcpListener::bind(&addr)
        .into_future()
        .and_then(|listener| {
            info!("Started listener on port 9100");
            listener.incoming().for_each(|stream| {
                info!("Incoming connection from {}", stream.peer_addr().unwrap());

                let (reader, _) = stream.split();

                let mut timestamp = time::SystemTime::now()
                    .duration_since(time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let filename = loop {
                    let file = env::current_exe()
                        .ok()
                        .and_then(|p| p.parent().map(|p| p.to_owned()))
                        .unwrap_or_else(|| PathBuf::new())
                        .join(format!("{}.spl", timestamp));
                    if !file.exists() {
                        break file;
                    } else {
                        timestamp += 1;
                    }
                };

                tokio::fs::File::create(filename.clone())
                    .and_then(|file| {
                        FileStream::new(Box::new(reader)).fold(file, |mut writer, bytes| {
                            writer.write_all(bytes.as_ref()).map(|_| writer)
                        })
                    })
                    .map(move |_| {
                        info!(
                            "Saved {} bytes into {}",
                            std::fs::metadata(&filename).unwrap().len(),
                            filename
                                .components()
                                .last()
                                .unwrap()
                                .as_os_str()
                                .to_string_lossy()
                        );
                    })
                    .map_err(|e| {
                        warn!("{}", e);
                        e
                    })
            })
        })
        .map_err(|e| {
            warn!("{}", e);
        })
}
