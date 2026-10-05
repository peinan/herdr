use super::*;

pub(super) fn decode_clipboard_payload(data: &str) -> Option<Vec<u8>> {
    use base64::Engine;

    base64::engine::general_purpose::STANDARD.decode(data).ok()
}

pub(super) fn forward_clipboard(clipboard: &clipboard_writer::ClipboardWriter, data: &str) -> bool {
    let Some(bytes) = decode_clipboard_payload(data) else {
        warn!("received invalid clipboard payload from server");
        return false;
    };
    clipboard.write(bytes);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use clipboard_writer::ClipboardWriter;

    const WAIT: Duration = Duration::from_secs(5);

    fn recording_writer() -> (ClipboardWriter, std::sync::mpsc::Receiver<Vec<u8>>) {
        let (calls, recorded) = std::sync::mpsc::channel();
        let (events, _) = tokio::sync::mpsc::channel(1);
        let writer = ClipboardWriter::new(
            false,
            Box::new(move |bytes: &[u8]| calls.send(bytes.to_vec()).is_ok()),
            Box::new(|_: &[u8]| panic!("copies must go through the native writer")),
            events,
        );
        (writer, recorded)
    }

    #[test]
    fn forwarded_clipboard_is_decoded_and_queued() {
        let (writer, recorded) = recording_writer();
        assert!(forward_clipboard(&writer, "dGVzdA=="));
        assert!(writer.flush(WAIT));
        assert_eq!(
            recorded.try_iter().collect::<Vec<_>>(),
            vec![b"test".to_vec()]
        );
    }

    #[test]
    fn invalid_clipboard_payload_writes_nothing() {
        let (writer, recorded) = recording_writer();
        assert!(!forward_clipboard(&writer, "not base64"));
        assert!(writer.flush(WAIT));
        assert!(recorded.try_recv().is_err());
    }
}
