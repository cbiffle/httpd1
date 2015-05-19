use std::io;
use std::result;

/// Errors that may kill off an HTTP request or connection.
#[derive(Debug)]
pub enum HttpError {
  ConnectionClosed,
  BadMethod,
  BadRequest,
  BadProtocol,
  SpanishInquisition,
  PreconditionFailed,
  NotImplemented(&'static [u8]),
  IoError(io::Error),
}

impl From<io::Error> for HttpError {
  fn from(e: io::Error) -> HttpError {
    HttpError::IoError(e)
  }
}

/// Alias for a Result in HttpError.
pub type Result<R> = result::Result<R, HttpError>;
