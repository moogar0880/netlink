use std::{
    error::Error as StdError,
    io::{self, Cursor},
    pin::Pin,
    task::{Context, Poll},
    collections::vec_deque::VecDeque,
};

use futures::{Sink, Stream};
use log::error;
use netlink_sys::{Socket, SocketAddr};

pub struct NetlinkFramed<T> {
    socket: Socket,
    codec: NetlinkCodec<T>,
    reader: Buffer,
    writer: Buffer,
    in_addr: SocketAddr,
    out_addr: SocketAddr,
    flushed: bool,
}

// FIXME: we should use bytes::BytesMut instead
pub struct Buffer(Cursor<Vec<u8>>);

impl Buffer {
    fn get_ref(&self) -> &[u8] {
        let pos = self.0.position();
        &self.0.get_ref()[pos..]
    }

    fn get_mut(&mut self) -> &mut [u8] {
        let pos = self.0.position();
        &mut self.0.get_mut()[pos..]
    }

    fn position(&self) -> usize {
        self.0.position()
    }

    fn set_position(&mut self, pos: usize) {
        self.0.set_position(pos)
    }

    fn extend(&mut self, additional: usize) {
        self.0.get_mut().resize(self.0.get_ref().len() + additional, 0)
    }

    fn len(&self) -> usize {
        self.0.get_ref().len()
    }

    fn remaining(&self) -> usize {
        self.len() - self.position()
    }
}


impl<T> Stream for NetlinkFramed<T>
{
    type Item = (NetlinkMessage<T>, SocketAddr);

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Self {
            ref mut codec,
            ref mut socket,
            ref mut in_addr,
            ref mut reader,
            ..
        } = Pin::get_mut(self);

        // In theory we shouldn't need a loop because according to the man, `recv()` only reads a
        // single datagram. Thus, simply calling `recv()` then decoding the datagram we read should
        // be enough. However, we've experienced that with netlink sockets (at least with the
        // NETLINK_ROUTE protocol) `recv()` sometimes receives multiple datagrams at once, hence this
        // weird loop.
        loop {
            match codec.decode(reader.get_mut()) {
                (n, Some(item)) => {
                    reader.set_position(n);
                    return Poll::Ready(Some((item, *in_addr)));
                }
                (_, None) => reader.set_position(0),
            }

            *in_addr = unsafe {
                match ready!(socket.poll_recv_from(cx, reader.get_mut())) {
                    Ok((n, addr)) => {
                        reader.set_position(n);
                        addr
                    }
                    Err(e) => {
                        error!("failed to read from netlink socket: {:?}", e);
                        return Poll::Ready(None);
                    }
                }
            };
        }
    }
}

impl<T> Sink<(NetlinkMessage<T>, SocketAddr)> for NetlinkFramed<T> {
    type Error = io:Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if !self.flushed {
            match self.poll_flush(cx)? {
                Poll::Ready(()) => {}
                Poll::Pending => return Poll::Pending,
            }
        }

        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: (C::Item, SocketAddr)) -> Result<(), Self::Error> {
        trace!("sending frame");
        let (frame, out_addr) = item;
        let pin = self.get_mut();

        if pin.writer.remaining() < framed.buffer_len() {
            if pin.writer.
            pin.writer.extend(

        }
        pin.codec.encode(frame, pin.writer.get_mut())?;
        pin.out_addr = out_addr;
        pin.flushed = false;
        trace!("frame encoded; length={}", pin.writer.len());
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.flushed {
            return Poll::Ready(Ok(()));
        }

        trace!("flushing frame; length={}", self.writer.len());
        let Self {
            ref mut socket,
            ref mut out_addr,
            ref mut writer,
            ..
        } = *self;

        let n = ready!(socket.poll_send_to(cx, &writer, &out_addr))?;
        trace!("written {}", n);

        let wrote_all = n == self.writer.len();
        self.writer.clear();
        self.flushed = true;

        let res = if wrote_all {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to write entire datagram to socket",
            )
            .into())
        };

        Poll::Ready(res)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }
}

// The theoritical max netlink packet size is 32KB for a netlink
// message since Linux 4.9 (16KB before). See:
// https://git.kernel.org/pub/scm/linux/kernel/git/davem/net-next.git/commit/?id=d35c99ff77ecb2eb239731b799386f3b3637a31e
const INITIAL_READER_CAPACITY: usize = 64 * 1024;
const INITIAL_WRITER_CAPACITY: usize = 8 * 1024;

impl<C> NetlinkFramed<C> {
    /// Create a new `NetlinkFramed` backed by the given socket and codec.
    ///
    /// See struct level documentation for more details.
    pub fn new(socket: Socket, codec: C) -> NetlinkFramed<C> {
        NetlinkFramed {
            socket,
            codec,
            out_addr: SocketAddr::new(0, 0),
            in_addr: SocketAddr::new(0, 0),
            reader: BytesMut::with_capacity(INITIAL_READER_CAPACITY),
            writer: BytesMut::with_capacity(INITIAL_WRITER_CAPACITY),
            flushed: true,
        }
    }

    /// Returns a reference to the underlying I/O stream wrapped by `Framed`.
    ///
    /// # Note
    ///
    /// Care should be taken to not tamper with the underlying stream of data
    /// coming in as it may corrupt the stream of frames otherwise being worked
    /// with.
    pub fn get_ref(&self) -> &Socket {
        &self.socket
    }

    /// Returns a mutable reference to the underlying I/O stream wrapped by
    /// `Framed`.
    ///
    /// # Note
    ///
    /// Care should be taken to not tamper with the underlying stream of data
    /// coming in as it may corrupt the stream of frames otherwise being worked
    /// with.
    pub fn get_mut(&mut self) -> &mut Socket {
        &mut self.socket
    }

    /// Consumes the `Framed`, returning its underlying I/O stream.
    pub fn into_inner(self) -> Socket {
        self.socket
    }
}
