/// A trait for waiting on a condition to be met.
///
/// This trait is used to abstract over different waiting strategies, such as spinning or sleeping, that can be used in concurrent programming scenarios. Implementors of this trait can define their own waiting behavior by implementing the `wait` method.
pub trait Wait {
    /// Creates a new instance of the implementor.
    fn new() -> Self;

    /// Waits for a condition to be met.
    fn wait(&mut self);
}

pub(crate) struct SpinWaiter;
impl Wait for SpinWaiter {
    fn new() -> Self {
        SpinWaiter
    }

    fn wait(&mut self) {
        crate::spin_loop();
    }
}
