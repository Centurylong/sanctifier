//! Minimal JSON-RPC 2.0 framing for LSP over stdio.
//!
//! LSP frames each message with an HTTP-like header block terminated by a
//! blank line, then exactly `Content-Length` bytes of UTF-8 JSON. Reading is
//! byte-oriented on purpose: a line-oriented reader will happily consume
//! bytes belonging to the next message's body when the payload contains a
//! newline, which every real diagnostic message does.

use std::io::{self, BufRead, Write};

/// Everything that can go wrong reading one frame.
#[derive(Debug)]
pub enum FrameError {
    /// The stream ended cleanly between messages — the client closed the pipe.
    Eof,
    Io(io::Error),
    /// The header block was malformed or `Content-Length` was missing.
    Protocol(String),
}

impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        FrameError::Io(e)
    }
}

/// Read one LSP frame and return its JSON body.
pub fn read_frame<R: BufRead>(reader: &mut R) -> Result<String, FrameError> {
    let mut content_length: Option<usize> = None;
    let mut saw_any_header = false;

    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return if saw_any_header {
                // Headers started but the stream died before the body — that is
                // a truncated message, not a clean shutdown, and the caller
                // should be able to tell the difference.
                Err(FrameError::Protocol(
                    "stream ended mid-header block".to_string(),
                ))
            } else {
                Err(FrameError::Eof)
            };
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break; // end of header block
        }
        saw_any_header = true;

        if let Some(value) = trimmed
            .split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .map(|(_, value)| value.trim())
        {
            content_length = Some(
                value
                    .parse()
                    .map_err(|_| FrameError::Protocol(format!("bad Content-Length: {value}")))?,
            );
        }
        // Content-Type and any unknown headers are ignored, per the spec.
    }

    let length = content_length
        .ok_or_else(|| FrameError::Protocol("missing Content-Length header".to_string()))?;

    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    String::from_utf8(body).map_err(|e| FrameError::Protocol(format!("body is not UTF-8: {e}")))
}

/// Write one LSP frame.
///
/// `Content-Length` counts bytes, not characters — a message with any
/// non-ASCII in it (a contract identifier, a path) would be truncated by the
/// client if this used `str::len()` on chars.
pub fn write_frame<W: Write>(writer: &mut W, body: &str) -> io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn framed(body: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
    }

    #[test]
    fn reads_a_single_frame() {
        let mut input = Cursor::new(framed(r#"{"jsonrpc":"2.0"}"#));
        assert_eq!(read_frame(&mut input).unwrap(), r#"{"jsonrpc":"2.0"}"#);
    }

    #[test]
    fn reads_consecutive_frames_without_bleeding() {
        // The regression this guards: a body containing a newline must not
        // cause the reader to swallow into the next frame.
        let mut bytes = framed("{\"a\":\"line one\nline two\"}");
        bytes.extend(framed(r#"{"b":2}"#));
        let mut input = Cursor::new(bytes);

        assert_eq!(
            read_frame(&mut input).unwrap(),
            "{\"a\":\"line one\nline two\"}"
        );
        assert_eq!(read_frame(&mut input).unwrap(), r#"{"b":2}"#);
    }

    #[test]
    fn ignores_unknown_headers_and_is_case_insensitive() {
        let body = r#"{"ok":true}"#;
        let raw = format!(
            "content-length: {}\r\nContent-Type: application/vscode-jsonrpc\r\n\r\n{}",
            body.len(),
            body
        );
        let mut input = Cursor::new(raw.into_bytes());
        assert_eq!(read_frame(&mut input).unwrap(), body);
    }

    #[test]
    fn clean_eof_between_messages_is_distinguishable() {
        let mut input = Cursor::new(Vec::new());
        assert!(matches!(read_frame(&mut input), Err(FrameError::Eof)));
    }

    #[test]
    fn missing_content_length_is_a_protocol_error() {
        let mut input = Cursor::new(b"X-Thing: 1\r\n\r\n{}".to_vec());
        assert!(matches!(
            read_frame(&mut input),
            Err(FrameError::Protocol(_))
        ));
    }

    #[test]
    fn content_length_counts_bytes_not_chars() {
        // "ü" is two bytes. A length computed over chars would truncate.
        let body = r#"{"m":"ü"}"#;
        let mut out = Vec::new();
        write_frame(&mut out, body).unwrap();

        let header = String::from_utf8_lossy(&out);
        assert!(
            header.starts_with(&format!("Content-Length: {}\r\n", body.len())),
            "header was {header:?}"
        );

        let mut round_trip = Cursor::new(out);
        assert_eq!(read_frame(&mut round_trip).unwrap(), body);
    }
}
