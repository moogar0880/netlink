#![allow(clippy::module_inception)]

use netlink_proto::{sys::Protocol, Connection};

mod handle;
pub use crate::handle::*;

mod errors;
pub use crate::errors::*;

use futures::channel::mpsc::UnboundedReceiver;
pub use netlink_packet_audit as packet;
pub use netlink_proto::sys;

use packet::{AuditMessage, NetlinkMessage};
use std::io;
use sys::SocketAddr;

#[allow(clippy::type_complexity)]
pub fn new_connection() -> io::Result<(
    Connection<AuditMessage>,
    Handle,
    UnboundedReceiver<(NetlinkMessage<AuditMessage>, SocketAddr)>,
)> {
    new_multicast_connection(0)
}

#[allow(clippy::type_complexity)]
pub fn new_multicast_connection(
    multicast_groups: u32,
) -> io::Result<(
    Connection<AuditMessage>,
    Handle,
    UnboundedReceiver<(NetlinkMessage<AuditMessage>, SocketAddr)>,
)> {
    let (conn, handle, messages) =
        netlink_proto::new_multicast_connection(Protocol::Audit, multicast_groups)?;
    Ok((conn, Handle::new(handle), messages))
}
