use futures::{Future, Stream};
use sqlx::{Pool, Postgres};
use tokio::sync::broadcast;
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

use std::{collections::BTreeMap, pin::Pin, task::Poll};

use super::{event::*, repo::*};
use crate::{error::*, primitives::*};

pub struct OutboxListener {
    pool: Pool<Postgres>,
    account_id: AccountId,
    last_sequence: EventSequence,
    latest_known: EventSequence,
    event_receiver: Pin<Box<BroadcastStream<OutboxEvent>>>,
    buffer_size: usize,
    cache: BTreeMap<EventSequence, OutboxEvent>,
    next_page_fut:
        Option<Pin<Box<dyn Future<Output = Result<Vec<OutboxEvent>, BriaError>> + Send>>>,
}

impl OutboxListener {
    pub(super) fn new(
        pool: &Pool<Postgres>,
        event_receiver: broadcast::Receiver<OutboxEvent>,
        account_id: AccountId,
        start_after: EventSequence,
        latest_known: EventSequence,
        buffer: usize,
    ) -> Self {
        Self {
            pool: pool.clone(),
            account_id,
            last_sequence: start_after,
            latest_known,
            event_receiver: Box::pin(BroadcastStream::new(event_receiver)),
            cache: BTreeMap::new(),
            next_page_fut: None,
            buffer_size: buffer,
        }
    }
}

impl OutboxListener {
    fn maybe_add_to_cache(&mut self, event: OutboxEvent) {
        if event.account_id == self.account_id {
            self.latest_known = self.latest_known.max(event.sequence);

            if event.sequence > self.last_sequence {
                if self.cache.insert(event.sequence, event).is_none()
                    && self.cache.len() > self.buffer_size
                {
                    self.cache.pop_last();
                }
            }
        }
    }
}

impl Stream for OutboxListener {
    type Item = OutboxEvent;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        // Poll page if present
        if let Some(fetch) = self.next_page_fut.as_mut() {
            match fetch.as_mut().poll(cx) {
                Poll::Ready(Ok(events)) => {
                    for event in events {
                        self.maybe_add_to_cache(event);
                    }
                    self.next_page_fut = None;
                }
                Poll::Ready(_) => {
                    self.next_page_fut = None;
                }
                Poll::Pending => (),
            }
        }
        // Poll as many events as we can come
        loop {
            match self.event_receiver.as_mut().poll_next(cx) {
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Ok(event))) => {
                    self.maybe_add_to_cache(event);
                }
                Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => (),
                Poll::Pending => break,
            }
        }

        while let Some((seq, event)) = self.cache.pop_first() {
            if seq <= self.last_sequence {
                continue;
            }
            if seq == self.last_sequence.next() {
                self.last_sequence = seq;
                self.next_page_fut = None;
                return Poll::Ready(Some(event));
            }
            self.cache.insert(seq, event);
        }

        if self.next_page_fut.is_none() && self.last_sequence < self.latest_known {
            self.next_page_fut = Some(Box::pin(OutboxRepo::load_next_page(
                self.pool.clone(),
                self.account_id,
                self.last_sequence,
                self.buffer_size,
            )));
            return self.poll_next(cx);
        }
        Poll::Pending
    }
}
