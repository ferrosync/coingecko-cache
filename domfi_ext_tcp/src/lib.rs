use std::{net, io};
use net2::TcpBuilder;
use net2::unix::UnixTcpBuilderExt;

pub fn create_listener(
    addr: net::SocketAddr,
    backlog: i32,
) -> io::Result<net::TcpListener> {
    let builder = match addr {
        net::SocketAddr::V4(_) => TcpBuilder::new_v4()?,
        net::SocketAddr::V6(_) => TcpBuilder::new_v6()?,
    };

    let socket = builder
        .reuse_address(true)?
        .reuse_port(true)?
        .bind(addr)?
        .listen(backlog)?;

    Ok(socket)
}

pub fn bind_to<A: net::ToSocketAddrs>(
    addr: A,
    backlog: i32,
) -> io::Result<Vec<net::TcpListener>> {
    let mut err = None;
    let mut success = false;
    let mut sockets = Vec::new();

    for addr in addr.to_socket_addrs()? {
        match create_listener(addr, backlog) {
            Ok(lst) => {
                success = true;
                sockets.push(lst);
            }
            Err(e) => err = Some(e),
        }
    }

    if !success {
        if let Some(e) = err.take() {
            Err(e)
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Can not bind to address.",
            ))
        }
    } else {
        Ok(sockets)
    }
}
