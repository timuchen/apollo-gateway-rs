use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use actix::Message;

use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;
use futures_util::StreamExt;

pub struct GroupedStream<K, S> {
    streams: Arc<Mutex<HashMap<K, S>>>,
    waker: Arc<AtomicWaker>,
}

impl<K, S> Clone for GroupedStream<K, S> {
    fn clone(&self) -> Self {
        Self {
            streams: Arc::clone(&self.streams),
            waker: Arc::clone(&self.waker)
        }
    }
}

impl<K, S> Default for GroupedStream<K, S> {
    fn default() -> Self {
        Self {
            streams: Default::default(),
            waker: Default::default(),
        }
    }
}

impl<K: Eq + Hash + Clone, S> GroupedStream<K, S> {
    #[inline]
    pub fn insert(&mut self, key: K, stream: S) {
        self.waker.wake();
        let mut guard = self.streams.lock().unwrap();
        guard.insert(key, stream);
    }

    #[inline]
    pub fn remove<Q: ?Sized>(&mut self, key: &Q)
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let mut guard = self.streams.lock().unwrap();
        guard.remove(key);
    }

    #[inline]
    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let mut guard = self.streams.lock().unwrap();
        guard.contains_key(key)
    }
}

pub enum StreamEvent<K, T> {
    Data(K, T),
    Complete(K),
}

impl<K, T> Message for StreamEvent<K, T> {
    type Result = ();
}

impl<K, T, S> Stream for GroupedStream<K, S>
where
    K: Eq + Hash + Clone + Unpin,
    S: Stream<Item = T> + Unpin,
{
    type Item = StreamEvent<K, T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.waker.register(cx.waker());
        let mut streams = self.streams.lock().unwrap();
        for (key, stream) in streams.iter_mut() {
            match stream.poll_next_unpin(cx) {
                Poll::Ready(Some(value)) => {
                    return Poll::Ready(Some(StreamEvent::Data(key.clone(), value)))
                }
                Poll::Ready(None) => {
                    let key = key.clone();
                    streams.remove(&key);
                    return Poll::Ready(Some(StreamEvent::Complete(key)));
                }
                Poll::Pending => {}
            }
        }

        Poll::Pending
    }
}
