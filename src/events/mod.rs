use compact_str::CompactString;
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AbundantisEvent {
    SourceAdded {
        source_id: super::source::SourceId,
    },
    SourceRemoved {
        source_id: super::source::SourceId,
    },
    VariablesChanged {
        source_id: super::source::SourceId,
        added: Vec<CompactString>,
        removed: Vec<CompactString>,
    },
    CacheInvalidated {
        scope: Option<super::workspace::WorkspaceContext>,
    },
}

pub trait EventSubscriber: Send + Sync {
    fn on_event(&self, event: &AbundantisEvent);
}

#[cfg(feature = "async")]
pub struct EventBus {
    subscribers: Arc<RwLock<Vec<Arc<dyn EventSubscriber>>>>,
    broadcast_tx: tokio::sync::broadcast::Sender<AbundantisEvent>,
}

#[cfg(feature = "async")]
impl EventBus {
    pub fn new(buffer_size: usize) -> Self {
        let (broadcast_tx, _) = tokio::sync::broadcast::channel(buffer_size.max(1));

        Self {
            subscribers: Arc::new(RwLock::new(Vec::new())),
            broadcast_tx,
        }
    }

    pub fn publish(&self, event: AbundantisEvent) {
        let subscribers = self.subscribers.read();
        for subscriber in subscribers.iter() {
            subscriber.on_event(&event);
        }

        let _ = self.broadcast_tx.send(event.clone());
    }

    pub async fn publish_async(&self, event: AbundantisEvent) {
        let subscribers = self.subscribers.read().clone();
        let event_clone = event.clone();

        let join_handle = tokio::task::spawn_blocking(move || {
            for subscriber in subscribers.iter() {
                subscriber.on_event(&event_clone);
            }
        });

        if let Err(e) = join_handle.await {
            tracing::error!("Async event subscriber failed: {:?}", e);
        }

        if self.broadcast_tx.send(event).is_err() {
            tracing::debug!("No receivers for event bus broadcast");
        }
    }

    pub fn subscribe(&self, subscriber: Arc<dyn EventSubscriber>) {
        let mut subscribers = self.subscribers.write();
        subscribers.push(subscriber);
    }

    pub fn unsubscribe(&self, subscriber: &Arc<dyn EventSubscriber>) {
        let mut subscribers = self.subscribers.write();
        subscribers.retain(|s| !Arc::ptr_eq(s, subscriber));
    }

    pub fn subscribe_channel(&self) -> tokio::sync::broadcast::Receiver<AbundantisEvent> {
        self.broadcast_tx.subscribe()
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().len()
    }

    pub fn receiver_count(&self) -> usize {
        self.broadcast_tx.receiver_count()
    }
}

#[cfg(feature = "async")]
impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            subscribers: Arc::clone(&self.subscribers),
            broadcast_tx: self.broadcast_tx.clone(),
        }
    }
}

#[cfg(not(feature = "async"))]
pub struct EventBus {
    subscribers: Arc<RwLock<Vec<Arc<dyn EventSubscriber>>>>,
}

#[cfg(not(feature = "async"))]
impl EventBus {
    pub fn new(_buffer_size: usize) -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn publish(&self, event: AbundantisEvent) {
        let subscribers = self.subscribers.read();
        for subscriber in subscribers.iter() {
            subscriber.on_event(&event);
        }
    }

    pub fn subscribe(&self, subscriber: Arc<dyn EventSubscriber>) {
        let mut subscribers = self.subscribers.write();
        subscribers.push(subscriber);
    }

    pub fn unsubscribe(&self, subscriber: &Arc<dyn EventSubscriber>) {
        let mut subscribers = self.subscribers.write();
        subscribers.retain(|s| !Arc::ptr_eq(s, subscriber));
    }

    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().len()
    }
}

#[cfg(not(feature = "async"))]
impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            subscribers: Arc::clone(&self.subscribers),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TestSubscriber {
        event_count: Arc<AtomicU32>,
    }

    impl TestSubscriber {
        fn new(event_count: Arc<AtomicU32>) -> Self {
            Self { event_count }
        }
    }

    impl EventSubscriber for TestSubscriber {
        fn on_event(&self, _event: &AbundantisEvent) {
            self.event_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_event_basics() {
        let bus = EventBus::new(100);
        let event_count = Arc::new(AtomicU32::new(0));
        let subscriber = Arc::new(TestSubscriber::new(Arc::clone(&event_count)));

        bus.subscribe(subscriber);

        let event = AbundantisEvent::CacheInvalidated { scope: None };
        bus.publish(event);

        assert_eq!(event_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_multiple_subscribers() {
        let bus = EventBus::new(100);
        let count1 = Arc::new(AtomicU32::new(0));
        let count2 = Arc::new(AtomicU32::new(0));

        bus.subscribe(Arc::new(TestSubscriber::new(Arc::clone(&count1))));
        bus.subscribe(Arc::new(TestSubscriber::new(Arc::clone(&count2))));

        bus.publish(AbundantisEvent::CacheInvalidated { scope: None });

        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_unsubscribe() {
        let bus = EventBus::new(100);
        let event_count = Arc::new(AtomicU32::new(0));
        let subscriber: Arc<dyn EventSubscriber> =
            Arc::new(TestSubscriber::new(Arc::clone(&event_count)));

        bus.subscribe(subscriber.clone());
        bus.unsubscribe(&subscriber);

        bus.publish(AbundantisEvent::CacheInvalidated { scope: None });

        assert_eq!(event_count.load(Ordering::SeqCst), 0);
    }
}
