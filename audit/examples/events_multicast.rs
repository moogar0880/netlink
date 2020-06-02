//! This example opens a multicast netlink socket, enables audit events, and
//! prints the events that are being received.

use audit::new_multicast_connection;
use futures::stream::StreamExt;
use netlink_packet_audit::constants::AUDIT_NLGRP_READLOG;

#[tokio::main]
async fn main() -> Result<(), String> {
    let (connection, mut handle, mut messages) =
        new_multicast_connection(AUDIT_NLGRP_READLOG).map_err(|e| format!("{}", e))?;

    tokio::spawn(connection);
    handle.enable_events().await.map_err(|e| format!("{}", e))?;

    env_logger::init();
    while let Some((msg, _)) = messages.next().await {
        println!("{:?}", msg);
    }
    Ok(())
}
