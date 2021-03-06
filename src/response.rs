//! HTTP response support.

use std::io;
use std::io::BufRead;
use std::time::SystemTime;

use crate::con::Connection;
use crate::error::{HttpError, Result};
use crate::file::OpenFile;
use crate::request::{Method, Protocol};

pub enum ContentEncoding {
    Gzip,
}

pub fn send(
    con: &mut Connection,
    method: Method,
    protocol: Protocol,
    now: SystemTime,
    encoding: Option<ContentEncoding>,
    if_modified_since: Option<&[u8]>,
    content_type: &[u8],
    resource: OpenFile,
) -> Result<()> {
    let mtime = httpdate::fmt_http_date(resource.mtime);

    let unmodified = if_modified_since == Some(mtime.as_bytes());

    if unmodified {
        con.log_other(b"note: not modified");
        start_response(con, protocol, now, b"304", b"not modified")?
    } else {
        start_response(con, protocol, now, b"200", b"OK")?
    }
    con.write(b"Content-Type: ")?;
    con.write(content_type)?;
    con.write(b"\r\n")?;

    con.write(b"Last-Modified: ")?;
    con.write(mtime.as_bytes())?;
    con.write(b"\r\n")?;

    if let Some(ContentEncoding::Gzip) = encoding {
        con.write(b"Content-Encoding: gzip\r\n")?
    }

    let send_content = method == Method::Get && !unmodified;

    let r = match protocol {
        Protocol::Http10 => send_unencoded(con, send_content, resource),
        Protocol::Http11 => send_chunked(con, send_content, resource),
    };

    con.flush_output()?;
    r
}

/// Signals the given error to the client.
///
/// Currently, this also closes the connection, though this seems like a
/// decision better left to the caller (TODO).
pub fn barf(
    mut con: Connection,
    protocol: Option<Protocol>,
    send_content: bool,
    error: HttpError,
) -> Result<()> {
    let (code, message) = match error.status() {
        None => return Ok(()),
        Some(pair) => pair,
    };

    start_response(
        &mut con,
        protocol.unwrap_or(Protocol::Http10),
        SystemTime::now(),
        code,
        message,
    )?;
    con.write(b"Content-Length: ")?;
    con.write_decimal(message.len() + 28)?; // length of HTML wrapper
    con.write(b"\r\n")?;

    if protocol == Some(Protocol::Http11) {
        con.write(b"Connection: close\r\n")?;
    }

    con.write(b"Content-Type: text/html\r\n\r\n")?;

    if send_content {
        con.write(b"<html><body>")?;
        con.write(message)?;
        con.write(b"</body></html>\r\n")?;
    }

    con.flush_output()
}

/// Sends a permanent redirect to the client.  The connection stays open.
pub fn redirect(
    con: &mut Connection,
    protocol: Protocol,
    send_content: bool,
    location: &[u8],
) -> Result<()> {
    let body = b"<html><body>moved permanently</body></html>";

    let now = SystemTime::now();
    start_response(con, protocol, now, b"301", b"moved permanently")?;
    con.write(b"Content-Length: ")?;
    con.write_decimal(body.len())?;
    con.write(b"\r\nLocation: ")?;
    con.write(location)?;
    con.write(b"\r\n")?;

    con.write(b"Content-Type: text/html\r\n\r\n")?;

    if send_content {
        con.write(body)?;
    }

    con.flush_output()?;

    match protocol {
        Protocol::Http10 => Err(HttpError::ConnectionClosed),
        Protocol::Http11 => Ok(()),
    }
}

fn send_unencoded(
    con: &mut Connection,
    send_content: bool,
    resource: OpenFile,
) -> Result<()> {
    con.write(b"Content-Length: ")?;
    con.write_decimal(resource.length as usize)?;
    con.write(b"\r\n\r\n")?;

    if send_content {
        let mut input = io::BufReader::with_capacity(1024, resource.file);
        loop {
            let count = {
                let chunk = input.fill_buf()?;
                if chunk.is_empty() {
                    break;
                }
                con.write(chunk)?;
                chunk.len()
            };
            input.consume(count);
        }
    }

    // We use unencoded responses for HTTP/1.0 clients, and we assume that
    // they don't use persistent connections.  This merits reconsideration (TODO).
    Err(HttpError::ConnectionClosed)
}

fn send_chunked(
    con: &mut Connection,
    send_content: bool,
    resource: OpenFile,
) -> Result<()> {
    con.write(b"Transfer-Encoding: chunked\r\n\r\n")?;

    if send_content {
        let mut input = io::BufReader::with_capacity(1024, resource.file);
        loop {
            let count = {
                let chunk = input.fill_buf()?;
                con.write_hex(chunk.len())?;
                con.write(b"\r\n")?;
                con.write(chunk)?;
                con.write(b"\r\n")?;

                chunk.len()
            };
            if count == 0 {
                // End of transfer.
                break;
            }

            input.consume(count)
        }
    }

    // Leave the connection open for more requests.
    Ok(())
}

/// Begins a response, printing the status line and a set of common headers.
/// The caller should follow up by adding any desired headers and then writing
/// a CRLF.
fn start_response(
    con: &mut Connection,
    prot: Protocol,
    now: SystemTime,
    code: &[u8],
    msg: &[u8],
) -> Result<()> {
    let now = httpdate::fmt_http_date(now);

    con.write(match prot {
        Protocol::Http10 => b"HTTP/1.0 ",
        Protocol::Http11 => b"HTTP/1.1 ",
    })?;
    con.write(code)?;
    con.write(b" ")?;
    con.write(msg)?;
    con.write(b"\r\nServer: abstract screaming\r\nDate: ")?;
    con.write(now.as_bytes())?;
    con.write(b"\r\n")?;
    Ok(())
}
