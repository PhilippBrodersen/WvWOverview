use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    sync::{Arc, Mutex},
    time::SystemTime,
};

use serde::de::DeserializeOwned;
use tokio::{sync::oneshot, time};

use crate::data::APIEndpoint;

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum Priority {
    High,
    Normal,
    Low,
}

struct ApiCall {
    priority: Priority,
    enqueue_time: SystemTime,
    url: String,
    job: Box<dyn FnOnce() -> tokio::task::JoinHandle<()> + Send>,
}

// Helper to convert Priority to a numeric value
impl Priority {
    fn value(&self) -> u8 {
        match self {
            Priority::High => 3,
            Priority::Normal => 2,
            Priority::Low => 1,
        }
    }
}

impl Ord for ApiCall {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare priority first
        let p = self.priority.value().cmp(&other.priority.value());
        if p == Ordering::Equal {
            // For same priority, older enqueue_time comes first
            other.enqueue_time.cmp(&self.enqueue_time)
        } else {
            p
        }
    }
}

impl PartialOrd for ApiCall {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ApiCall {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.enqueue_time == other.enqueue_time
    }
}

impl Eq for ApiCall {}

#[derive(Clone)]
pub struct ApiQueue {
    queue: Arc<Mutex<BinaryHeap<ApiCall>>>,
    delay: std::time::Duration,
}

impl ApiQueue {
    pub fn new(delay: std::time::Duration) -> Self {
        let q = Self {
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            delay,
        };
        q.start_queue();
        q
    }

    pub fn clear(&self) {
        let mut q = self.queue.lock().unwrap();
        q.clear();
    }

    pub fn enqueue<T>(
        &self,
        end_point: APIEndpoint,
        priority: Priority,
    ) -> impl Future<Output = Option<T>>
    where
        T: 'static + Send + DeserializeOwned,
    {
        let (tx, rx) = oneshot::channel::<Option<T>>();
        let url_clone = end_point.to_string().clone();
        let job = move || {
            tokio::spawn(async move {
                if let Ok(response) = reqwest::get(url_clone).await
                    && let Ok(t) = response.json::<T>().await
                {
                    let _ = tx.send(Some(t));
                }
            })
        };

        let call = ApiCall {
            priority,
            enqueue_time: SystemTime::now(),
            url: end_point.to_string(),
            job: Box::new(job),
        };

        self.queue.lock().unwrap().push(call);

        async move { rx.await.unwrap_or(None) }
    }

    fn start_queue(&self) {
        let queue_clone = self.queue.clone();
        let delay = self.delay;

        tokio::spawn(async move {
            let mut interval = time::interval(delay);
            loop {
                interval.tick().await;

                let call_opt = {
                    let mut q = queue_clone.lock().unwrap();
                    q.pop()
                };

                if let Some(call) = call_opt {
                    (call.job)();
                }
            }
        });
    }
}
