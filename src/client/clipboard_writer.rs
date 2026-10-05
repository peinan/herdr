//! Clipboard writes for the client event loop.
//!
//! Native clipboard tools (pbcopy, wl-copy, xclip) are separate processes, so
//! waiting for them stalls the loop. Native writes run in order on one worker
//! thread instead. OSC 52 goes to stdout, which only the loop writes, so a
//! failed native write returns to the loop to be written as OSC 52.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use tracing::{debug, warn};

use super::ClientLoopEvent;

/// How long shutdown waits for queued native writes, so a copy made right
/// before detaching still lands.
const FLUSH_TIMEOUT: Duration = Duration::from_millis(500);

type NativeWrite = Box<dyn Fn(&[u8]) -> bool + Send>;
type Osc52Write = Box<dyn Fn(&[u8])>;

enum Job {
    Write(Vec<u8>),
    Flush(mpsc::Sender<()>),
}

pub(super) struct ClipboardWriter {
    /// `None` when copies go straight to OSC 52 (SSH, WSL, VS Code) or the
    /// worker could not start.
    jobs: Option<mpsc::Sender<Job>>,
    /// Failed native writes handed back to the loop and not yet written as OSC 52.
    pending_fallbacks: Arc<AtomicUsize>,
    write_osc52: Osc52Write,
}

impl ClipboardWriter {
    pub(super) fn spawn(events: tokio::sync::mpsc::Sender<ClientLoopEvent>) -> Self {
        Self::new(
            crate::selection::should_prefer_osc52(),
            Box::new(crate::platform::write_clipboard),
            Box::new(crate::selection::write_osc52_sequence),
            events,
        )
    }

    pub(super) fn new(
        prefer_osc52: bool,
        native: NativeWrite,
        write_osc52: Osc52Write,
        events: tokio::sync::mpsc::Sender<ClientLoopEvent>,
    ) -> Self {
        let pending_fallbacks = Arc::new(AtomicUsize::new(0));
        let jobs = if prefer_osc52 {
            None
        } else {
            let (jobs, queue) = mpsc::channel();
            let worker_pending = pending_fallbacks.clone();
            match std::thread::Builder::new()
                .name("clipboard-writer".into())
                .spawn(move || run_worker(queue, native, worker_pending, events))
            {
                Ok(_) => Some(jobs),
                Err(err) => {
                    warn!(err = %err, "failed to spawn clipboard writer; copies use OSC 52");
                    None
                }
            }
        };
        Self {
            jobs,
            pending_fallbacks,
            write_osc52,
        }
    }

    /// Hands `bytes` to the system clipboard without waiting for a native tool.
    pub(super) fn write(&self, bytes: Vec<u8>) {
        let Some(jobs) = &self.jobs else {
            (self.write_osc52)(&bytes);
            return;
        };
        if let Err(mpsc::SendError(Job::Write(bytes))) = jobs.send(Job::Write(bytes)) {
            warn!("clipboard writer stopped; writing the copy as OSC 52");
            (self.write_osc52)(&bytes);
        }
    }

    /// Writes a copy the worker could not place natively. Runs on the loop,
    /// which owns stdout.
    pub(super) fn write_fallback(&self, bytes: &[u8]) {
        (self.write_osc52)(bytes);
        release_pending(&self.pending_fallbacks);
    }

    /// Waits for every write queued so far. Returns false on timeout.
    fn flush(&self, timeout: Duration) -> bool {
        let Some(jobs) = &self.jobs else {
            return true;
        };
        let (done, completion) = mpsc::channel();
        if jobs.send(Job::Flush(done)).is_err() {
            return true;
        }
        !matches!(
            completion.recv_timeout(timeout),
            Err(mpsc::RecvTimeoutError::Timeout)
        )
    }

    #[cfg(test)]
    pub(super) fn inert() -> Self {
        Self {
            jobs: None,
            pending_fallbacks: Arc::default(),
            write_osc52: Box::new(|_| {}),
        }
    }
}

impl Drop for ClipboardWriter {
    fn drop(&mut self) {
        if !self.flush(FLUSH_TIMEOUT) {
            warn!("clipboard writes were still queued at shutdown");
        }
    }
}

fn run_worker(
    jobs: mpsc::Receiver<Job>,
    native: NativeWrite,
    pending_fallbacks: Arc<AtomicUsize>,
    events: tokio::sync::mpsc::Sender<ClientLoopEvent>,
) {
    for job in jobs {
        let bytes = match job {
            Job::Write(bytes) => bytes,
            Job::Flush(done) => {
                let _ = done.send(());
                continue;
            }
        };
        // While an older copy waits for OSC 52, a native write would land
        // first and then be overwritten, so this one queues behind it instead.
        // Native is tried again once the loop has written every fallback.
        if pending_fallbacks.load(Ordering::Acquire) == 0 {
            if native(&bytes) {
                continue;
            }
            debug!(
                bytes = bytes.len(),
                "native clipboard write failed; falling back to OSC 52"
            );
        }
        pending_fallbacks.fetch_add(1, Ordering::AcqRel);
        if events
            .blocking_send(ClientLoopEvent::ClipboardFallback(bytes))
            .is_err()
        {
            release_pending(&pending_fallbacks);
            warn!("client loop stopped; dropping a clipboard write");
        }
    }
}

fn release_pending(pending_fallbacks: &AtomicUsize) {
    let _ = pending_fallbacks.fetch_update(Ordering::AcqRel, Ordering::Acquire, |pending| {
        pending.checked_sub(1)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    const WAIT: Duration = Duration::from_secs(5);

    type Recorded = Rc<RefCell<Vec<Vec<u8>>>>;

    fn osc52_recorder() -> (Osc52Write, Recorded) {
        let written = Recorded::default();
        let sink = written.clone();
        let write: Osc52Write =
            Box::new(move |bytes: &[u8]| sink.borrow_mut().push(bytes.to_vec()));
        (write, written)
    }

    fn next_fallback(events: &mut tokio::sync::mpsc::Receiver<ClientLoopEvent>) -> Option<Vec<u8>> {
        match events.try_recv() {
            Ok(ClientLoopEvent::ClipboardFallback(bytes)) => Some(bytes),
            _ => None,
        }
    }

    #[test]
    fn native_writes_run_in_order_on_the_worker_thread() {
        let (calls, recorded) = mpsc::channel();
        let native: NativeWrite = Box::new(move |bytes: &[u8]| {
            let thread = std::thread::current().name().map(str::to_owned);
            let _ = calls.send((thread, bytes.to_vec()));
            true
        });
        let (osc52, written) = osc52_recorder();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(8);
        let writer = ClipboardWriter::new(false, native, osc52, events_tx);

        for text in ["one", "two", "three"] {
            writer.write(text.as_bytes().to_vec());
        }
        assert!(writer.flush(WAIT));

        let worker = Some("clipboard-writer".to_owned());
        assert_eq!(
            recorded.try_iter().collect::<Vec<_>>(),
            vec![
                (worker.clone(), b"one".to_vec()),
                (worker.clone(), b"two".to_vec()),
                (worker, b"three".to_vec()),
            ]
        );
        assert!(written.borrow().is_empty());
        assert!(next_fallback(&mut events).is_none());
    }

    #[test]
    fn failed_native_write_holds_later_writes_until_its_fallback_is_written() {
        let (calls, recorded) = mpsc::channel();
        let native: NativeWrite = Box::new(move |bytes: &[u8]| {
            let _ = calls.send(bytes.to_vec());
            bytes != b"busy"
        });
        let (osc52, written) = osc52_recorder();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(8);
        let writer = ClipboardWriter::new(false, native, osc52, events_tx);

        writer.write(b"busy".to_vec());
        writer.write(b"second".to_vec());
        assert!(writer.flush(WAIT));
        assert_eq!(
            recorded.try_iter().collect::<Vec<_>>(),
            vec![b"busy".to_vec()]
        );
        let busy = next_fallback(&mut events).expect("failed write returns to the loop");
        let second = next_fallback(&mut events).expect("later write queues behind it");
        assert_eq!(busy, b"busy");
        assert_eq!(second, b"second");

        writer.write_fallback(&busy);
        writer.write(b"third".to_vec());
        assert!(writer.flush(WAIT));
        assert_eq!(recorded.try_iter().count(), 0);
        let third = next_fallback(&mut events).expect("still behind a pending fallback");
        assert_eq!(third, b"third");

        writer.write_fallback(&second);
        writer.write_fallback(&third);
        assert_eq!(
            *written.borrow(),
            vec![b"busy".to_vec(), b"second".to_vec(), b"third".to_vec()]
        );

        writer.write(b"after".to_vec());
        assert!(writer.flush(WAIT));
        assert_eq!(
            recorded.try_iter().collect::<Vec<_>>(),
            vec![b"after".to_vec()]
        );
        assert!(next_fallback(&mut events).is_none());
    }

    #[test]
    fn preferred_osc52_writes_inline_without_native() {
        let (calls, recorded) = mpsc::channel();
        let native: NativeWrite = Box::new(move |bytes: &[u8]| {
            let _ = calls.send(bytes.to_vec());
            true
        });
        let (osc52, written) = osc52_recorder();
        let (events_tx, mut events) = tokio::sync::mpsc::channel(8);
        let writer = ClipboardWriter::new(true, native, osc52, events_tx);

        writer.write(b"remote".to_vec());
        assert_eq!(*written.borrow(), vec![b"remote".to_vec()]);
        drop(writer);
        assert!(recorded.try_recv().is_err());
        assert!(next_fallback(&mut events).is_none());
    }

    #[test]
    fn dropping_the_writer_waits_for_a_slow_native_write() {
        let landed = Arc::new(AtomicBool::new(false));
        let worker_landed = landed.clone();
        let native: NativeWrite = Box::new(move |_: &[u8]| {
            std::thread::sleep(Duration::from_millis(100));
            worker_landed.store(true, Ordering::Release);
            true
        });
        let (osc52, _) = osc52_recorder();
        let (events_tx, _events) = tokio::sync::mpsc::channel(8);
        let writer = ClipboardWriter::new(false, native, osc52, events_tx);

        writer.write(b"detach".to_vec());
        drop(writer);
        assert!(landed.load(Ordering::Acquire));
    }

    #[test]
    fn dropping_the_writer_stops_waiting_for_a_hung_native_write() {
        let (release, released) = mpsc::channel::<()>();
        let native: NativeWrite = Box::new(move |_: &[u8]| {
            let _ = released.recv();
            true
        });
        let (osc52, _) = osc52_recorder();
        let (events_tx, _events) = tokio::sync::mpsc::channel(8);
        let writer = ClipboardWriter::new(false, native, osc52, events_tx);

        writer.write(b"hung".to_vec());
        let started = Instant::now();
        drop(writer);
        let waited = started.elapsed();
        assert!(waited >= FLUSH_TIMEOUT);
        assert!(waited < WAIT);
        drop(release);
    }
}
