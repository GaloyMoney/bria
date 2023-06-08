use futures::{FutureExt, Stream};
use tokio::{sync::broadcast, task::JoinHandle};
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

use std::{collections::BTreeMap, pin::Pin, task::Poll};

use super::{augmentation::*, error::OutboxError, event::*, repo::*};
use crate::primitives::*;

pub struct OutboxListener {
    repo: OutboxRepo,
    account_id: AccountId,
    augmenter: Option<Augmenter>,
    next_to_augment: Option<OutboxEvent<Augmentation>>,
    augmentation_handle: Option<JoinHandle<Result<Augmentation, OutboxError>>>,
    last_sequence: EventSequence,
    latest_known: EventSequence,
    event_receiver: Pin<Box<BroadcastStream<OutboxEvent<WithoutAugmentation>>>>,
    buffer_size: usize,
    cache: BTreeMap<EventSequence, OutboxEvent<WithoutAugmentation>>,
    next_page_handle:
        Option<JoinHandle<Result<Vec<OutboxEvent<WithoutAugmentation>>, OutboxError>>>,
}

impl OutboxListener {
    pub(super) fn new(
        repo: OutboxRepo,
        augmenter: Option<Augmenter>,
        event_receiver: broadcast::Receiver<OutboxEvent<WithoutAugmentation>>,
        account_id: AccountId,
        start_after: EventSequence,
        latest_known: EventSequence,
        buffer: usize,
    ) -> Self {
        Self {
            repo,
            augmenter,
            next_to_augment: None,
            augmentation_handle: None,
            account_id,
            last_sequence: start_after,
            latest_known,
            event_receiver: Box::pin(BroadcastStream::new(event_receiver)),
            cache: BTreeMap::new(),
            next_page_handle: None,
            buffer_size: buffer,
        }
    }
}

impl OutboxListener {
    fn maybe_add_to_cache(&mut self, event: OutboxEvent<WithoutAugmentation>) {
        if event.account_id == self.account_id {
            self.latest_known = self.latest_known.max(event.sequence);

            if event.sequence > self.last_sequence
                && self.cache.insert(event.sequence, event).is_none()
                && self.cache.len() > self.buffer_size
            {
                self.cache.pop_last();
            }
        }
    }

    fn poll_next_from_stream(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<OutboxEvent<Augmentation>>> {
        // Poll page if present
        if let Some(fetch) = self.next_page_handle.as_mut() {
            match fetch.poll_unpin(cx) {
                Poll::Ready(Ok(Ok(events))) => {
                    for event in events {
                        self.maybe_add_to_cache(event);
                    }
                    self.next_page_handle = None;
                }
                Poll::Ready(_) => {
                    self.next_page_handle = None;
                }
                Poll::Pending => (),
            }
        }
        // Poll as many events as we can come
        loop {
            match self.event_receiver.as_mut().poll_next(cx) {
                Poll::Ready(None) => {
                    if let Some(handle) = self.next_page_handle.take() {
                        handle.abort();
                    }
                    return Poll::Ready(None);
                }
                Poll::Ready(Some(Ok(event))) => {
                    self.maybe_add_to_cache(event);
                }
                Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => (),
                Poll::Pending => break,
            }
        }

        if self.next_to_augment.is_some() {
            return Poll::Pending;
        }

        while let Some((seq, event)) = self.cache.pop_first() {
            if seq <= self.last_sequence {
                continue;
            }
            if seq == self.last_sequence.next() {
                self.last_sequence = seq;
                if let Some(handle) = self.next_page_handle.take() {
                    handle.abort();
                }
                return Poll::Ready(Some(OutboxEvent::<Augmentation>::from(event)));
            }
            self.cache.insert(seq, event);
        }

        if self.next_page_handle.is_none() && self.last_sequence < self.latest_known {
            let repo = self.repo.clone();
            let account_id = self.account_id;
            let last_sequence = self.last_sequence;
            let buffer_size = self.buffer_size;
            self.next_page_handle = Some(tokio::spawn(async move {
                repo.load_next_page(account_id, last_sequence, buffer_size)
                    .await
            }));
            return self.poll_next(cx);
        }
        Poll::Pending
    }
}

impl Stream for OutboxListener {
    type Item = OutboxEvent<Augmentation>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.as_mut().poll_next_from_stream(cx) {
            res if self.augmenter.is_none() => {
                return res;
            }
            Poll::Ready(Some(event)) if self.next_to_augment.is_none() => {
                self.next_to_augment = Some(event);
            }
            res if self.next_to_augment.is_none() => {
                return res;
            }
            _ => (),
        }

        if let Some(handle) = self.augmentation_handle.as_mut() {
            match handle.poll_unpin(cx) {
                Poll::Ready(Ok(Ok(augmentation))) => {
                    self.augmentation_handle = None;
                    let mut next_event = self
                        .next_to_augment
                        .take()
                        .expect("missing netxt_to_augment");
                    next_event.augmentation = Some(augmentation);
                    return Poll::Ready(Some(next_event));
                }
                Poll::Ready(_) => {
                    self.augmentation_handle = None;
                }
                Poll::Pending => (),
            }
        }

        if self.augmentation_handle.is_none() && self.next_to_augment.is_some() {
            let augmenter = self.augmenter.as_ref().expect("missing augmenter").clone();
            let account_id = self.account_id;
            let payload = self
                .next_to_augment
                .as_ref()
                .expect("missing next_to_augment")
                .payload
                .clone();
            self.augmentation_handle = Some(tokio::spawn(async move {
                augmenter.load_augmentation(account_id, payload).await
            }));
            return self.poll_next(cx);
        }

        Poll::Pending
    }
}
