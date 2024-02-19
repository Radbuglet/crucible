use std::{error::Error, fmt, future::Future, mem, path::Path};

use anyhow::Context;

use super::extension::ExtensionFor;

// === AnyhowAsyncExt === //

pub trait AnyhowAsyncExt: Sized + ExtensionFor<Result<Self::Ok, Self::Err>> {
    type Ok;
    type Err: 'static + Send + Sync + Error;

    fn with_context_async<F, Fut>(self, f: F) -> impl Future<Output = anyhow::Result<Self::Ok>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = String>,
    {
        async {
            match self.into_v() {
                Ok(v) => Ok(v),
                Err(e) => Err(anyhow::Error::new(e).context(f().await)),
            }
        }
    }
}

impl<T, E: 'static + Send + Sync + Error> AnyhowAsyncExt for Result<T, E> {
    type Ok = T;
    type Err = E;
}

// === Anyhow Std Adapter === //

#[repr(transparent)]
pub struct AnyhowStdAdapter(pub anyhow::Error);

impl AnyhowStdAdapter {
    pub fn wrap_ref(v: &anyhow::Error) -> &Self {
        unsafe { std::mem::transmute(v) }
    }
}

impl fmt::Debug for AnyhowStdAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for AnyhowStdAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for AnyhowStdAdapter {}

pub trait AnyhowAdapterExt: Sized + ExtensionFor<anyhow::Error> {
    fn into_std_err(self) -> AnyhowStdAdapter {
        AnyhowStdAdapter(self.into_v())
    }

    fn as_std_err(&self) -> &AnyhowStdAdapter {
        AnyhowStdAdapter::wrap_ref(self.v())
    }
}

impl AnyhowAdapterExt for anyhow::Error {}

// === Anyhow Freeze Adapter === //

pub trait AnyhowFreezeExt: Sized + Error {
    fn freeze(self) -> anyhow::Error {
        match self.source() {
            Some(parent) => parent.freeze().context(format!("{self}")),
            None => anyhow::Error::msg(format!("{self}")),
        }
    }
}

impl<T: Error> AnyhowFreezeExt for T {}

// === MultiError === //

#[derive(Debug)]
pub struct MultiError {
    name: String,
    errors: Vec<anyhow::Error>,
}

impl Error for MultiError {}

impl fmt::Display for MultiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to complete {:?} because of {} fatal error{}",
            self.name,
            self.errors.len(),
            if self.errors.len() == 1 { "" } else { "s" }
        )
    }
}

impl MultiError {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            errors: Vec::new(),
        }
    }

    #[track_caller]
    pub fn report(&mut self, error: anyhow::Error) {
        log::error!("Error occurred during {:?}:\n{error:?}", self.name);
        self.errors.push(error);
    }

    pub fn maybe_report<T>(&mut self, res: anyhow::Result<T>) -> Option<T> {
        match res {
            Ok(val) => Some(val),
            Err(err) => {
                self.report(err);
                None
            }
        }
    }

    pub fn finish(self) -> anyhow::Result<()> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow::Error::new(self))
        }
    }

    pub fn reset(&mut self, name: impl Into<String>) -> anyhow::Result<()> {
        mem::replace(self, Self::new(name)).finish()
    }
}

// === scope_err === //

pub fn scope_err<T>(f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    f()
}

pub async fn scope_err_async<T, Fut: Future<Output = anyhow::Result<T>>>(
    f: impl FnOnce() -> Fut,
) -> anyhow::Result<T> {
    f().await
}

// === FS Errors === //

pub async fn tokio_read_file_anyhow(what: &str, path: impl AsRef<Path>) -> anyhow::Result<Vec<u8>> {
    let path = path.as_ref();
    tokio::fs::read(path).await.with_context(|| {
        format!(
            "failed to read {what} (path: {:?})",
            match std::env::current_dir() {
                Ok(cwd) => path_clean::clean(cwd.join(path)),
                Err(_) => path.to_path_buf(),
            },
        )
    })
}
