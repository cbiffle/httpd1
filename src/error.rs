//! HTTP protocol errors

use std::io;
use std::result;

/// Errors that may kill off an HTTP request or connection.
#[derive(Debug)]
pub enum HttpError {
  /// The client has gone away or sent us something that leads us to believe
  /// that they'd like to.  This is the one error that can't reasonably be
  /// reported back to the client.
  ConnectionClosed,
  /// The client used a method other than GET or HEAD.
  BadMethod,
  /// The initial request-line was malformed, e.g. contained too few or too many
  /// space-separated tokens.  Note that any request from an HTTP/0.9 client
  /// will be detected as `BadRequest`.
  BadRequest,
  /// The protocol sent by the client was unrecognized.
  BadProtocol,
  /// The client sent the 'Expect' header, which we were ironically not
  /// expecting.
  SpanishInquisition,
  /// The client sent 'If-Match' or 'If-Unmodified-Since' headers, and we are
  /// treating the test they described as having failed.
  PreconditionFailed,
  /// The client has tried to use an aspect of HTTP that we don't implement,
  /// but which doesn't have a specific HTTP status code.
  NotImplemented(&'static [u8]),
  /// The requested resource was not found (404), or we're acting like it wasn't
  /// due to permissions mismatch.
  NotFound,
  /// For convenience, `io::Error`s can be propagated as `HttpError`s.
  /// We treat them as internal server errors; any *expected* I/O errors should
  /// be coerced into another error type.
  IoError(io::Error),
}

impl From<io::Error> for HttpError {
  fn from(e: io::Error) -> HttpError {
    HttpError::IoError(e)
  }
}

/// Alias for a Result in HttpError.
pub type Result<R> = result::Result<R, HttpError>;
