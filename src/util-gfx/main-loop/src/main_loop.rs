use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};

// N.B. We only create the window on `resumed` because...
//
// > It's recommended that applications should only initialize their graphics context and create a
// > window after they have received their first Resumed event. Some systems (specifically Android)
// > won't allow applications to create a render surface until they are resumed.
//
// ...according to winit 0.30.0's documentation on `resumed`.
//
// Technically, this implementation ignores events from before `resumed` but that *probably* won't
// be a problem in practice and I'd really rather not think about it... :(
pub fn run_app_with_init<T, A>(
    event_loop: EventLoop<T>,
    handler: impl FnOnce(&ActiveEventLoop) -> anyhow::Result<A>,
) -> anyhow::Result<()>
where
    T: 'static,
    A: ApplicationHandler<T>,
{
    enum InnerApp<L, A> {
        WaitingForResume(Option<L>),
        FailedInit(anyhow::Error),
        Ready(A),
    }

    impl<L, A> InnerApp<L, A> {
        fn state(&mut self) -> Option<&mut A> {
            match self {
                Self::Ready(state) => Some(state),
                _ => None,
            }
        }
    }

    impl<T, L, A> ApplicationHandler<T> for InnerApp<L, A>
    where
        T: 'static,
        L: FnOnce(&ActiveEventLoop) -> anyhow::Result<A>,
        A: ApplicationHandler<T>,
    {
        fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
            if let Some(state) = self.state() {
                state.new_events(event_loop, cause);
            }
        }

        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            match self {
                Self::WaitingForResume(handler) => {
                    let handler = handler.take().unwrap();
                    let state = handler(event_loop);

                    match state {
                        Ok(state) => {
                            *self = Self::Ready(state);
                        }
                        Err(err) => {
                            *self = Self::FailedInit(err);
                            event_loop.exit();
                        }
                    }
                }
                Self::FailedInit(_) => {
                    // (no op)
                }
                Self::Ready(state) => state.resumed(event_loop),
            }
        }

        fn user_event(&mut self, event_loop: &ActiveEventLoop, event: T) {
            if let Some(state) = self.state() {
                state.user_event(event_loop, event);
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            if let Some(state) = self.state() {
                state.window_event(event_loop, window_id, event);
            }
        }

        fn device_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            device_id: DeviceId,
            event: DeviceEvent,
        ) {
            if let Some(state) = self.state() {
                state.device_event(event_loop, device_id, event);
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if let Some(state) = self.state() {
                state.about_to_wait(event_loop);
            }
        }

        fn suspended(&mut self, event_loop: &ActiveEventLoop) {
            if let Some(state) = self.state() {
                state.suspended(event_loop);
            }
        }

        fn exiting(&mut self, event_loop: &ActiveEventLoop) {
            if let Some(state) = self.state() {
                state.exiting(event_loop);
            }
        }

        fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
            if let Some(state) = self.state() {
                state.memory_warning(event_loop);
            }
        }
    }

    let mut app = InnerApp::WaitingForResume(Some(handler));

    match event_loop.run_app(&mut app) {
        Ok(()) => match app {
            InnerApp::WaitingForResume(_) | InnerApp::Ready(_) => Ok(()),
            InnerApp::FailedInit(err) => Err(err),
        },
        Err(err) => Err(anyhow::anyhow!(err).context("`event_loop` creation error ocurred")),
    }
}
