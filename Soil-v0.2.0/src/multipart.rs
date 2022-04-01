use std::{pin::Pin, task::Poll};

use anyhow::Error;
use bytes::{Bytes, BytesMut};
use futures::{AsyncRead, Stream};

static SEP: Bytes = Bytes::from_static(b"--");
static CRLF: Bytes = Bytes::from_static(b"\r\n");

pub struct Part {
    body: PartBody,
    content_disposition: Bytes,
}

impl Part {
    pub fn new<B>(body: PartBody, name: B) -> Self
    where
        B: Into<Bytes> + Send + 'static,
    {
        let pre = b"Content-Disposition: form-data; name=\"";
        let name: Bytes = name.into();
        let mut disposition = BytesMut::with_capacity(2 + pre.len() + name.len() + 5);
        disposition.extend_from_slice(&CRLF);
        disposition.extend_from_slice(pre);
        disposition.extend_from_slice(&name);
        disposition.extend_from_slice(&CRLF);
        disposition.extend_from_slice(&CRLF);
        Part {
            body,
            content_disposition: disposition.freeze(),
        }
    }
}

pub enum PartBody {
    Simple(Bytes),
    Read(Pin<Box<dyn AsyncRead + Send>>),
}

pub enum StreamState {
    Next,
    BoundaryDone,
    DispositionDone(Part),
    BodyDone,
    Done,
}

pub struct MultipartStream {
    boundary: Bytes,
    parts: Box<dyn Iterator<Item = Part> + Send>,
    state: StreamState,
}

impl MultipartStream {
    pub fn new<B>(boundary: B, parts: Vec<Part>) -> Self
    where
        B: Into<Bytes> + Send + 'static,
    {
        let boundary = boundary.into();
        let mut bytes = BytesMut::with_capacity(boundary.len() + 2);
        bytes.extend_from_slice(&SEP);
        bytes.extend_from_slice(&boundary);
        MultipartStream {
            boundary: bytes.freeze(),
            parts: Box::new(parts.into_iter()),
            state: StreamState::Next,
        }
    }
}

impl Stream for MultipartStream {
    type Item = Result<Bytes, Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match &mut self.state {
            StreamState::Next => {
                self.state = StreamState::BoundaryDone;
                Poll::Ready(Some(Ok(self.boundary.clone())))
            }
            StreamState::BoundaryDone => {
                if let Some(part) = self.parts.next() {
                    let disposition = part.content_disposition.clone();
                    self.state = StreamState::DispositionDone(part);
                    Poll::Ready(Some(Ok(disposition)))
                } else {
                    self.state = StreamState::Done;
                    Poll::Ready(Some(Ok(SEP.clone())))
                }
            }
            StreamState::DispositionDone(ref mut part) => match part.body {
                PartBody::Simple(ref body) => {
                    let body = body.clone();
                    self.state = StreamState::BodyDone;
                    Poll::Ready(Some(Ok(body)))
                }
                PartBody::Read(ref mut read) => {
                    let mut buf = [0_u8; 512];
                    match read.as_mut().poll_read(cx, &mut buf) {
                        Poll::Ready(n) => {
                            // not sure if this is correct
                            let n = n?;
                            if n == 0 {
                                self.state = StreamState::BodyDone;
                                // restart state machine by asking to be polled again
                                cx.waker().wake_by_ref();
                                Poll::Pending
                            } else {
                                // can we avoid the copy?
                                Poll::Ready(Some(Ok(Bytes::copy_from_slice(&buf[..n]))))
                            }
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
            },
            StreamState::BodyDone => {
                self.state = StreamState::Next;
                Poll::Ready(Some(Ok(CRLF.clone())))
            }
            StreamState::Done => Poll::Ready(None),
        }
    }
}
