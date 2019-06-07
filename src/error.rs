//! HTTP protocol errors

use std::io;
use std::result;

use std::error::Error;

/// Errors that may kill off an HTTP request or connection.
#[derive(Debug)]
pub enum HttpError {
    /// The client has gone away or sent us something that leads us to believe
    /// that they'd like to.  This is the one error that can't reasonably be
    /// reported back to the client.  We'll occasionally coerce other results
    /// into this error to force a connection closed.
    ConnectionClosed,

    /// 400 - The initial request-line was malformed, e.g. contained too few or
    /// too many space-separated tokens.  Note that any request from an HTTP/0.9
    /// client will be detected as `BadRequest`.
    BadRequest,

    /// 404 - The requested resource was not found (404), or we're acting like it
    /// wasn't due to permissions mismatch.  The context message will be logged
    /// but not revealed to the client, since it may let them learn about the
    /// presence of unreadable files that they might try to access through other
    /// means.
    NotFound(&'static [u8]),

    /// 408 - The client didn't send data within the time we were willing to wait.
    RequestTimeout,

    /// 412 - The client sent 'If-Match' or 'If-Unmodified-Since' headers, and we
    /// are treating the test they described as having failed.
    PreconditionFailed,

    /// 417 - The client sent the 'Expect' header, which we were ironically not
    /// expecting.
    SpanishInquisition,

    /// 501 - The client used a method other than GET or HEAD.
    BadMethod,

    /// 501 - The client has tried to use an aspect of HTTP that we don't
    /// implement, but which doesn't have a specific HTTP status code.
    /// The context message will be revealed to the client, because there seems
    /// little risk in doing so.
    NotImplemented(&'static [u8]),

    /// 505 - The protocol sent by the client was unrecognized.
    BadProtocol,

    /// For convenience, `io::Error`s can be propagated as `HttpError`s.
    /// We treat them as internal server errors (500); any *expected* I/O errors
    /// should be coerced into another error type.
    IoError(io::Error),
}

impl HttpError {
    /// Returns the numeric HTTP status code appropriate for this error, along
    /// with a short ASCII-encoded explanatory message.
    pub fn status<'a>(&'a self) -> Option<(&'a [u8], &'a [u8])> {
        use self::HttpError::*;

        match *self {
            ConnectionClosed => None,
            BadRequest => Some((b"400", b"bad request")),
            NotFound(_) => Some((b"404", b"not found")),
            RequestTimeout => Some((b"408", b"type faster")),
            PreconditionFailed => Some((b"412", b"precondition failed")),
            SpanishInquisition => Some((b"417", b"unexpected")),
            IoError(ref e) => Some((b"500", e.description().as_bytes())),
            BadMethod => Some((b"501", b"bad method")),
            NotImplemented(m) => Some((b"501", m)),
            BadProtocol => Some((b"505", b"bad protocol")),
        }
    }

    /// Returns a description of this error appropriate for a trusted audience,
    /// such as a log file.
    pub fn log_message<'a>(&'a self) -> Option<&'a [u8]> {
        use self::HttpError::*;

        match *self {
            ConnectionClosed => None,
            BadRequest => Some(b"bad request"),
            NotFound(m) => Some(m),
            RequestTimeout => None,
            PreconditionFailed => Some(b"precondition failed"),
            SpanishInquisition => Some(b"unexpected"),
            IoError(ref e) => Some(e.description().as_bytes()),
            BadMethod => Some(b"bad method"),
            NotImplemented(m) => Some(m),
            BadProtocol => Some(b"bad protocol"),
        }
    }
}

impl From<io::Error> for HttpError {
    fn from(e: io::Error) -> HttpError {
        use std::io::ErrorKind::*;

        match e.kind() {
            NotFound => HttpError::NotFound(b"io not found"),
            PermissionDenied => HttpError::NotFound(b"io permission denied"),
            TimedOut => HttpError::RequestTimeout,
            _ => HttpError::IoError(e),
        }
    }
}

/// Alias for a Result in HttpError.
pub type Result<R> = result::Result<R, HttpError>;
