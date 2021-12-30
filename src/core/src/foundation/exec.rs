use crate::util::error::AnyResult;
use anyhow::Context;
use futures::executor::ThreadPool;
use futures::task::SpawnExt;
use num_cpus::get as get_cpu_count;
use std::future::Future;

pub struct Executor {
	core_pool: ThreadPool,
}

impl Default for Executor {
	fn default() -> Self {
		Self::new(ExecutorConfig::default()).unwrap()
	}
}

impl Executor {
	pub fn new(config: ExecutorConfig) -> AnyResult<Self> {
		let core_pool = {
			let mut builder = ThreadPool::builder();
			builder.pool_size(config.cores);

			if let Some(stack_size) = config.stack_size {
				builder.stack_size(stack_size);
			}

			if let Some(prefix) = &config.name_prefix {
				builder.name_prefix(prefix);
			}

			builder.create().context("failed to create thread pool")?
		};

		Ok(Self { core_pool })
	}

	pub fn spawn_core<T>(&self, fut: T) -> impl Future<Output = T::Output>
	where
		T: 'static + Send + Future,
		T::Output: Send,
	{
		self.core_pool.spawn_with_handle(fut).unwrap()
	}
}

#[derive(Debug, Clone)]
pub struct ExecutorConfig {
	cores: usize,
	name_prefix: Option<String>,
	stack_size: Option<usize>,
}

impl ExecutorConfig {
	pub fn count_logical_cores() -> usize {
		get_cpu_count()
	}
}

impl Default for ExecutorConfig {
	fn default() -> Self {
		Self {
			cores: Self::count_logical_cores(),
			name_prefix: None,
			stack_size: None,
		}
	}
}
