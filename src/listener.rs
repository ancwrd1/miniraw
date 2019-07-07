use std::{env, io, path::PathBuf, time};

use futures::{
    future::{IntoFuture, Loop},
    stream::Stream,
    Future,
};
use log::{info, warn};
use tokio::{io::AsyncRead, net::TcpListener};

pub fn start_raw_listener() -> impl Future<Item = (), Error = ()> {
    let addr = "0.0.0.0:9100".parse().unwrap();
    TcpListener::bind(&addr)
        .into_future()
        .and_then(|listener| {
            info!("Started listener on port 9100");
            listener.incoming().for_each(|stream| {
                info!("Incoming connection from {}", stream.peer_addr().unwrap());

                let (reader, _) = stream.split();

                let timestamp = time::SystemTime::now()
                    .duration_since(time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                // get file name atomically in a future loop
                let target = futures::future::loop_fn(timestamp, |timestamp| {
                    let filename = format!("{}.spl", timestamp);
                    let filepath = env::current_exe()
                        .ok()
                        .and_then(|p| p.parent().map(|p| p.to_owned()))
                        .unwrap_or_else(|| PathBuf::new())
                        .join(&filename);

                    tokio::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(filepath)
                        .and_then(|writer| Ok(Loop::Break((writer, filename))))
                        .or_else(move |e| {
                            if e.kind() == io::ErrorKind::AlreadyExists {
                                Ok(Loop::Continue(timestamp + 1))
                            } else {
                                Err(e)
                            }
                        })
                });

                target
                    .and_then(|(writer, filename)| {
                        tokio::io::copy(reader, writer).map(move |(bytes, _, _)| {
                            info!("Saved {} bytes into {}", bytes, filename);
                        })
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
