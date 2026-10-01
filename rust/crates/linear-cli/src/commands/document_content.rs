//! Document text intentionally differs from strict comment-body/bulk-ID files.
use crate::{
    error::{AppError, AppErrorKind},
    text::js_space,
};
use std::{io::Read, sync::mpsc, time::Duration};
/// Deno.readTextFile retains an initial BOM and replaces malformed UTF8.
pub fn decode_file(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
pub fn split_stdin(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let content = text
        .split(|ch| ch == ',' || js_space(ch))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!content.is_empty()).then_some(content)
}
/// Source's100ms whole-read body race; unlike source create's pending-read
/// lifetime, a detached standard worker does not hold process exit open.
/// DOC-STDIN-HELD-EXIT names that deliberate lifetime boundary only.
pub fn optional_stdin<R: Read + Send + 'static>(mut reader: R) -> Result<Option<String>, AppError> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = reader.read_to_end(&mut bytes).map(|_| bytes);
        match sender.send(result) {
            Ok(()) => (),
            Err(mpsc::SendError(_)) => { /* Receiver's optional deadline elapsed. */ }
        }
    });
    match receiver.recv_timeout(Duration::from_millis(100)) {
        Ok(Ok(bytes)) => Ok(split_stdin(&bytes)),
        Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(AppError::new(
            AppErrorKind::Invariant,
            "document stdin reader unexpectedly disappeared",
        )),
    }
}
pub fn edited_body(text: &str) -> Option<String> {
    let text = text.trim_matches(js_space);
    (!text.is_empty()).then(|| text.to_owned())
}
