use core::fmt::{self, Write as _};

// === WriteFromFn === //

#[derive(Copy, Clone)]
pub struct WriteFromFn<F>(pub F)
where
    F: FnMut(&str) -> fmt::Result;

impl<F> fmt::Write for WriteFromFn<F>
where
    F: FnMut(&str) -> fmt::Result,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0(s)
    }
}

// === Processors === //

pub struct ProcessFmt<P, F>(pub P, pub F);

impl<P: Processor, F: fmt::Debug> fmt::Debug for ProcessFmt<P, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(&mut self.0.new_writer(f), "{:?}", self.1)
    }
}

impl<P: Processor, F: fmt::Display> fmt::Display for ProcessFmt<P, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(&mut self.0.new_writer(f), "{}", self.1)
    }
}

pub trait Processor {
    fn new_writer(&self, target: &mut impl fmt::Write) -> impl fmt::Write;
}
