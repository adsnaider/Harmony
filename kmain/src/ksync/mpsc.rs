//! `std::sync::mpsc` equivalent for our kernel.

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use core::cell::RefCell;
use core::future::poll_fn;
use core::task::{Context, Poll, Waker};

#[derive(Debug)]
struct Channel<T> {
    buffer: VecDeque<T>,
    senders: usize,
    receiver_closed: bool,
    waker: Option<Waker>,
}

/// A sender portion of an unbounded multi-producer, single-consumer channel.
#[derive(Debug)]
pub struct Sender<T> {
    channel: Rc<RefCell<Channel<T>>>,
}

/// A receiving portion of an unbounded multi-producer, single-consumer channel.
#[derive(Debug)]
pub struct Receiver<T> {
    channel: Rc<RefCell<Channel<T>>>,
}

/// Create a new unbounded multi-producer, single-consumer channel.
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let channel = Rc::new(RefCell::new(Channel {
        buffer: VecDeque::new(),
        senders: 0,
        receiver_closed: false,
        waker: None,
    }));
    let sender = Sender::new(Rc::clone(&channel));
    let receiver = Receiver::new(channel);
    (sender, receiver)
}

impl<T> Sender<T> {
    fn new(channel: Rc<RefCell<Channel<T>>>) -> Self {
        channel.borrow_mut().senders += 1;
        Self { channel }
    }

    /// Sends a message through the channel, returning an Err with the original value if the
    /// channel is already closed.
    pub fn send(&self, t: T) -> Result<(), T> {
        let mut channel = self.channel.borrow_mut();
        if channel.receiver_closed {
            return Err(t);
        }
        channel.buffer.push_back(t);
        if let Some(waker) = channel.waker.take() {
            waker.wake();
        }
        Ok(())
    }

    /// Returns true if the channel is closed.
    pub fn is_closed(&self) -> bool {
        self.channel.borrow().receiver_closed
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Sender::new(Rc::clone(&self.channel))
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.channel.borrow_mut().senders -= 1;
    }
}

/// Error receiving a message in the mpsc channel.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RecvError {
    /// All senders disconnected.
    Disconnected,
}

impl<T> Receiver<T> {
    fn new(channel: Rc<RefCell<Channel<T>>>) -> Self {
        Self { channel }
    }

    /// Get a new message.
    ///
    /// This method returns an error if there are no new messages and the senders have all
    /// disconnected.
    ///
    /// # Cancel Safety
    ///
    /// This method is cancel-safe.
    pub async fn recv(&mut self) -> Result<T, RecvError> {
        poll_fn(|cx| self.poll_recv(cx)).await
    }

    /// Like `recv` but the underlying poll function.
    pub fn poll_recv(&mut self, cx: &mut Context) -> Poll<Result<T, RecvError>> {
        let mut channel = self.channel.borrow_mut();
        if let Some(t) = channel.buffer.pop_front() {
            Poll::Ready(Ok(t))
        } else {
            if channel.senders == 0 {
                Poll::Ready(Err(RecvError::Disconnected))
            } else {
                channel.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    /// Synchronously receive a new message if available.
    pub fn try_recv(&mut self) -> Result<Option<T>, RecvError> {
        let mut channel = self.channel.borrow_mut();
        if let Some(t) = channel.buffer.pop_front() {
            Ok(Some(t))
        } else {
            if channel.senders == 0 {
                Err(RecvError::Disconnected)
            } else {
                Ok(None)
            }
        }
    }

    /// Close the channel to prevent new message from coming in.
    pub fn close(&mut self) {
        self.channel.borrow_mut().receiver_closed = true;
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.close();
    }
}
