use std::{any::Any, collections::VecDeque, io};

use tokio::sync::Mutex;
use wasi_common::{
    file::{FdFlags, FileType},
    ErrorExt, WasiFile,
};

// === StdStream === //

type WasiResult<T> = Result<T, wasi_common::Error>;

#[wiggle::async_trait]
pub trait StdStreamHandler: 'static + Send + Sync {
    async fn write(&self, bytes: &[io::IoSlice<'_>]) -> u64;
}

#[derive(Default)]
pub struct StdStream<H>(pub H);

#[wiggle::async_trait]
impl<H: StdStreamHandler> WasiFile for StdStream<H> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn get_filetype(&self) -> WasiResult<FileType> {
        Ok(FileType::CharacterDevice)
    }

    async fn get_fdflags(&self) -> WasiResult<FdFlags> {
        Ok(FdFlags::APPEND)
    }

    fn isatty(&self) -> bool {
        true
    }

    async fn write_vectored<'a>(&self, bufs: &[io::IoSlice<'a>]) -> WasiResult<u64> {
        Ok(self.0.write(bufs).await)
    }

    async fn write_vectored_at<'a>(
        &self,
        _bufs: &[io::IoSlice<'a>],
        _offset: u64,
    ) -> WasiResult<u64> {
        Err(wasi_common::Error::seek_pipe())
    }

    async fn seek(&self, _pos: std::io::SeekFrom) -> WasiResult<u64> {
        Err(wasi_common::Error::seek_pipe())
    }
}

// === LogStreamHandler === //

pub type StdLogStream = StdStream<LogStreamHandler>;

pub fn create_std_log_stream(target: impl Into<String>) -> StdLogStream {
    StdStream(LogStreamHandler::new(target))
}

pub struct LogStreamHandler {
    target: String,
    buffer: Mutex<VecDeque<u8>>,
}

impl LogStreamHandler {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            buffer: Mutex::default(),
        }
    }
}

#[wiggle::async_trait]
impl StdStreamHandler for LogStreamHandler {
    async fn write(&self, bytes: &[io::IoSlice<'_>]) -> u64 {
        let mut buf = self.buffer.lock().await;
        let mut written = 0;

        for byte in bytes.iter().flat_map(|s| s.iter()).copied() {
            if byte == b'\n' {
                log::info!(
                    target: &self.target,
                    "{}",
                    String::from_utf8_lossy(&buf.iter().copied().collect::<Vec<_>>())
                        .trim_end_matches('\r')
                );
                buf.clear();
            } else {
                buf.push_back(byte);
            }
            written += 1;
        }

        written
    }
}
