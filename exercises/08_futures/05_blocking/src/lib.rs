// TODO: the `echo` server uses non-async primitives.
//  When running the tests, you should observe that it hangs, due to a
//  deadlock between the caller and the server.
//  Use `spawn_blocking` inside `echo` to resolve the issue.
use std::io::{Read, Write};
use tokio::net::TcpListener;

pub async fn echo(listener: TcpListener) -> Result<(), anyhow::Error> {
    loop {
        let (socket, _) = listener.accept().await?;
        let mut socket = socket.into_std()?;
        socket.set_nonblocking(false)?;
        let mut buffer = Vec::new();
        tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
            socket.read_to_end(&mut buffer)?;
            socket.write_all(&buffer)?;
            Ok(())
        });
        // if you add `.await??`, the function will wait for the blocking task to return
        // a value before proceeding, meaning that it will not be ready to accept new
        // incoming connections until the blocking task has finished.
        
        // this defeats a little bit the purpose of creating an expensive task that runs
        // on its own, but sometimes the information coming from that task is needed
        // by the parent task for further processing.
        // if waiting that blocking task is a task which is not the main one, then,
        // depending on the context, it might be totally fine.

        // more on `spawn_blocking` and `await`:
        // https://users.rust-lang.org/t/tokio-calling-sync-operation-from-async-and-awaiting-still-blocks-the-thread/85990
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::panic;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::task::JoinSet;

    async fn bind_random() -> (TcpListener, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        (listener, addr)
    }

    #[tokio::test]
    async fn test_echo() {
        let (listener, addr) = bind_random().await;
        tokio::spawn(echo(listener));

        let requests = vec![
            "hello here we go with a long message",
            "world",
            "foo",
            "bar",
        ];
        let mut join_set = JoinSet::new();

        for request in requests {
            join_set.spawn(async move {
                let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();
                let (mut reader, mut writer) = socket.split();

                // Send the request
                writer.write_all(request.as_bytes()).await.unwrap();
                // Close the write side of the socket
                writer.shutdown().await.unwrap();

                // Read the response
                let mut buf = Vec::with_capacity(request.len());
                reader.read_to_end(&mut buf).await.unwrap();
                assert_eq!(&buf, request.as_bytes());
            });
        }

        while let Some(outcome) = join_set.join_next().await {
            if let Err(e) = outcome {
                if let Ok(reason) = e.try_into_panic() {
                    panic::resume_unwind(reason);
                }
            }
        }
    }
}
