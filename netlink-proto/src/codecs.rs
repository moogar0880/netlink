use failure::Fail;
use std::fmt::Debug;
use std::io;
use std::marker::PhantomData;

use bytes::{BufMut, BytesMut};
use netlink_packet_core::{
    NetlinkBuffer, NetlinkDeserializable, NetlinkMessage, NetlinkSerializable,
};

// This is a poor's man implementation of tokio's tokio_util::codec::Codec. The reason we're not
// using it directly is that it relies on `bytes` >= 0.5, where BytesMut uses potentially
// un-initialized memory, which we're not ready to deal with yet. See:
// https://github.com/tokio-rs/bytes/issues/317
//
// Since we're not working with BytesMut but &[u8], this codec is currently a lot less useful but
// we're keeping it because we're planning to move back to using `bytes` eventually
pub struct NetlinkCodec<T> {
    phantom: PhantomData<T>,
}

impl<T> Default for NetlinkCodec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> NetlinkCodec<T> {
    pub fn new() -> Self {
        NetlinkCodec {
            phantom: PhantomData,
        }
    }
}

impl<T> NetlinkCodec<NetlinkMessage<T>>
where
    T: NetlinkSerializable<T> + NetlinkDeserializable<T> + Debug + Eq + PartialEq + Clone,
{
    fn decode(&mut self, src: &mut [u8]) -> (usize, Option<NetlinkMessage<T>>) {
        debug!("NetlinkCodec: decoding next message");
        let mut bytes_read = 0;

        loop {
            if src.len() == 0 {
                trace!("buffer is empty");
                return (0, None);
            }

            // The audit messages are sometimes truncated. We found two such cases:
            //
            // 1. the length of the header is not included. In such case, the difference is exactly
            //    16 bytes. Ref: https://github.com/mozilla/libaudit-go/issues/24
            //
            // 2. some rule message have some padding for alignment which is not taken into account
            //    in the buffer length. In such cases the difference should be less than 4 bytes,
            //    because netlink messages are 4 bytes aligned. Ref:
            //    https://github.com/linux-audit/audit-userspace/issues/78
            //
            // Note that our check assumes that there is only one message in the buffer (if we have
            // several, the difference will likely be greater 16 bytes). We shouldn't have multiple
            // messages because `decode()` is called right after `recv()` which (according to the
            // man) receives a single message. We've noticed that for the NETLINK_ROUTE protocol,
            // `recv()` could actually receive multiple datagrams at once, but with NETLINK_AUDIT it
            // seems `recv()` has the expected behavior.
            //
            // FIXME: maybe that gymnastic belongs to `netlink-packet-audit` instead.
            #[cfg(feature = "workaround-audit-bug")]
            {
                if let Ok(buf) = NetlinkBuffer::new_checked(&mut src[bytes_read..]) {
                    // NOTE: we assume that there's only one message in `src`
                    if (src.len() as isize - buf.length() as isize) <= 16 {
                        warn!("found what looks like a truncated audit packet");
                        warn!(
                            "setting message length field to {} instead of {}",
                            len,
                            buf.length()
                        );
                        buf.set_length(len as u32);
                    }
                }
            }

            match NetlinkBuffer::new_checked(&src[bytes_read..]) {
                Ok(buf) => {
                    let buf_end = bytes_read + buf.length() as usize;
                    let parsed = NetlinkMessage::<T>::deserialize(&src[bytes_read..buf_end]);
                    match parsed {
                        Ok(packet) => {
                            trace!("<<< {:?}", packet);
                            return (buf_end, Some(packet));
                        }
                        Err(e) => {
                            let mut error_string =
                                format!("failed to decode packet {:#x?}", buf.into_inner());
                            for cause in Fail::iter_chain(&e) {
                                error_string += &format!(": {}", cause);
                            }
                            error!("{}", error_string);
                            bytes_read = buf_end;
                            // continue looping, there may be more datagrams in the buffer
                        }
                    }
                }
                Err(e) => {
                    // We either received a truncated packet, or the packet if malformed (invalid
                    // length field). In both case, we can't decode the datagram, and we cannot find
                    // the start of the next one (if any). The only solution is to clear the buffer
                    // and potentially lose some datagrams.
                    error!("failed to decode datagram: {:?}: {:#x?}.", e, src);
                    return (src.len(), None);
                }
            }
        }
    }

    fn encode(&mut self, msg: NetlinkMessage<T>, buf: &mut [u8]) -> Result<usize, io::Error> {
        let message_len = msg.buffer_len();
        let buffer_len = buf.len();

        if buffer_len < message_len {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "message is {} bytes, but only {} bytes left in the buffer",
                    message_len, buffer_len
                ),
            ));
        }

        msg.serialize(&mut buf[..message_len]);
        trace!(">>> {:?}", msg);
        Ok(message_len)
    }
}
