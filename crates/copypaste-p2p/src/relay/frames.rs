//! A relay WebSocket as a frame transport: one binary message per Noise
//! message, so the session above it cannot tell it from TCP.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use futures_util::{Sink, SinkExt, Stream, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tokio_util::bytes::{Bytes, BytesMut};

pub(super) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(super) struct WsFrames(pub(super) Socket);

/// The relay's socket address, for logs only.
pub(super) fn relay_addr(socket: &Socket) -> SocketAddr {
    match socket.get_ref() {
        MaybeTlsStream::Plain(stream) => stream.peer_addr().ok(),
        MaybeTlsStream::Rustls(stream) => stream.get_ref().0.peer_addr().ok(),
        _ => None,
    }
    .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 0)))
}

fn to_io(error: tokio_tungstenite::tungstenite::Error) -> io::Error {
    io::Error::other(error)
}

impl Stream for WsFrames {
    type Item = io::Result<BytesMut>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            return Poll::Ready(match ready!(self.0.poll_next_unpin(cx)) {
                Some(Ok(Message::Binary(data))) => Some(Ok(BytesMut::from(data.as_slice()))),
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => continue,
                Some(Ok(Message::Close(_))) | None => None,
                Some(Ok(Message::Text(_))) => Some(Err(io::ErrorKind::InvalidData.into())),
                Some(Err(error)) => Some(Err(to_io(error))),
            });
        }
    }
}

impl Sink<Bytes> for WsFrames {
    type Error = io::Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.poll_ready_unpin(cx).map_err(to_io)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Bytes) -> io::Result<()> {
        self.0
            .start_send_unpin(Message::Binary(item.to_vec()))
            .map_err(to_io)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.poll_flush_unpin(cx).map_err(to_io)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.poll_close_unpin(cx).map_err(to_io)
    }
}
