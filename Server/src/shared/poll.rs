#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Poll<T, E> {
    Pending,
    Ready(T),
    Failed(E),
    Finished,
}

impl<T, E> Poll<T, E> {
    #[inline(always)]
    pub fn is_pending(&self) -> bool {
        matches!(self, Poll::Pending)
    }
}
