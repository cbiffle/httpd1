//! The core HTTP server, which ties the other modules together.

use std::ffi;
use std::os::unix::ffi::OsStrExt;
use std::time::SystemTime;

use super::con::Connection;
use super::error::*;
use super::request::{Method, Protocol, Request};
use super::response::ContentEncoding;
use super::{file, filetype, path, percent, request, response};

pub fn serve(remote: String) -> Result<()> {
    let mut c = Connection::new(remote);

    loop {
        // Process requests.
        let req = match request::read(&mut c) {
            Ok(r) => r,
            Err(e) => return response::barf(c, None, true, e),
        };

        // Back up two pieces before we consume the request.
        let protocol = req.protocol;
        let method = req.method;

        if let Err(error) = serve_request(&mut c, req) {
            // Try to report this to the client.  Error reporting is best-effort.
            let _ =
                response::barf(c, Some(protocol), method == Method::Get, error);
            return Ok(());
        }

        // Otherwise, carry on accepting requests.
    }
}

fn serve_request(con: &mut Connection, req: Request) -> Result<()> {
    // The request may not have included a Host, but we need to use it to
    // generate a file path.  Tolerate Host's absence for HTTP/1.0 requests
    // by replacing it with the simulated host "0".
    let host = match req.host {
        None => match req.protocol {
            Protocol::Http10 => vec![b'0'],

            // HTTP 1.1 requests must include a host, one way or another.
            Protocol::Http11 => return Err(HttpError::BadRequest),
        },
        Some(ref h) => {
            // TODO: we're copying this to preserve the bytes sent by the client
            // again.
            let mut h = h.clone();
            normalize_host(&mut h);
            h
        }
    };

    // TODO: doing the percent escaping in-place is seeming less snazzy now that
    // we're having to copy the input path.
    let mut path = req.path.clone();
    percent::unescape(&mut path)?;

    // TODO: probably better to percent-escape into this buffer.
    let mut file_path = path::sanitize(
        b"./".iter().chain(&host).chain(b"/").chain(&path).cloned(),
    );

    let now = SystemTime::now();
    let content_type = filetype::from_path(&file_path[..]);
    if let file::FileOrDir::File(mut resource) =
        open_resource(con, &file_path[..], None)?
    {
        let mut encoding = None;

        // If that worked, see if there's *also* a GZIPped alternate with accessible
        // permissions.
        if req.accept_gzip {
            file_path.extend(b".gz".iter().cloned());
            if let Ok(file::FileOrDir::File(alt)) =
                open_resource(con, &file_path[..], Some(b"gzipped"))
            {
                // It must be at least as recent as the primary, or we'll assume it's
                // stale clutter and ignore it.
                if alt.mtime >= resource.mtime {
                    // Rewrite the file and length, but leave everything else
                    // (particularly mtime).
                    con.log_other(b"note: serving gzipped");
                    resource.file = alt.file;
                    resource.length = alt.length;
                    encoding = Some(ContentEncoding::Gzip)
                }
            }
        }

        response::send(
            con,
            req.method,
            req.protocol,
            now,
            encoding,
            req.if_modified_since.as_ref().map(Vec::as_slice),
            &content_type[..],
            resource,
        )
    } else {
        // It's a dir.
        if let Some(ref orig_host) = req.host {
            let url: Vec<_> = b"http://"
                .iter()
                .chain(orig_host)
                .chain(&req.path)
                .chain(b"/".iter())
                .cloned()
                .collect();

            return response::redirect(
                con,
                req.protocol,
                req.method == Method::Get,
                &url[..],
            );
        } else {
            Err(HttpError::NotFound(b"cannot redirect"))
        }
    }
}

fn open_resource(
    con: &mut Connection,
    path: &[u8],
    context: Option<&'static [u8]>,
) -> Result<file::FileOrDir> {
    let result = file::safe_open(ffi::OsStr::from_bytes(path));

    match result {
        Ok(file::FileOrDir::File(_)) => {
            con.log(path, context, b"success");
        }

        Ok(file::FileOrDir::Dir) => {
            con.log(path, context, b"directory redirect");
        }

        Err(ref e) => {
            if let Some(message) = e.log_message() {
                con.log(path, context, message);
            }
        }
    }

    result
}

// If the client provided a host, we must normalize it for use as a directory
// name: downcase it and strip off the port, if any.
fn normalize_host(host: &mut Vec<u8>) {
    for i in 0..host.len() {
        let c = host[i];
        if c == b':' {
            host.truncate(i);
            return;
        } else {
            host[i] = c.to_ascii_lowercase()
        }
    }
}
