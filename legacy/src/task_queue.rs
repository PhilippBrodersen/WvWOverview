use futures::{FutureExt, TryFutureExt};
use std::collections::HashMap;
use std::error::Error;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::{fmt, future::Future};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot, Mutex, Semaphore, SemaphorePermit};
use tokio::time::Instant;

use crate::gw2api::fetch_all_wvw_guild_ids;

async fn log_error<E: fmt::Debug>(err: E) {
    let debug_str = format!("{err:?}\n");

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("error.log")
        .await
    {
        if let Err(e) = file.write_all(debug_str.as_bytes()).await {
            eprintln!("Failed to write to log file: {e:?}");
        }
    } else {
        eprintln!("Failed to open error.log");
    }
}

#[derive(Debug)]
pub enum TaskError {
    DeserializationError(String),
    DatabaseError(sqlx::Error),
    HttpError(reqwest::Error),
    Tokio(tokio::sync::oneshot::error::RecvError),
    Other(String)
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::DeserializationError(msg) => write!(f, "Not Found Error: {msg}"),
            TaskError::DatabaseError(err) => write!(f, "IO Error: {err}"),
            TaskError::HttpError(error) => write!(f, "HTTP Error: {error}"),
            TaskError::Other(err) => write!(f, " Error: {err}"),
            TaskError::Tokio(recv_error) => write!(f, "ahhh {recv_error}")
        }
    }
}

impl Error for TaskError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TaskError::DatabaseError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for TaskError {
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        TaskError::Tokio(e)
    }
}

impl From<sqlx::Error> for TaskError {
    fn from(e: sqlx::Error) -> Self {
        TaskError::DatabaseError(e)
    }
}

impl From<reqwest::Error> for TaskError {
    fn from(e: reqwest::Error) -> Self {
        TaskError::HttpError(e)
    }
}

type BoxedJob = Pin<Box<dyn Future<Output = ()> + Send>>;

pub struct ProcessingQueue {
    sender: mpsc::Sender<BoxedJob>,
    api_semaphore: Arc<Semaphore>,
    last_api_call: Arc<Mutex<Instant>>,
}

impl ProcessingQueue {
    pub fn new(buffer: usize) -> Self {
        let (tx, mut rx) = mpsc::channel::<BoxedJob>(buffer);
        let api_semaphore = Arc::new(Semaphore::new(1));
        let last_api_call = Arc::new(Mutex::new(
            Instant::now() - std::time::Duration::from_millis(200),
        ));

        tokio::spawn(async move {
            while let Some(job) = rx.recv().await {
                // Each job is spawned individually
                tokio::spawn(job);
            }
        });

        Self {
            sender: tx,
            api_semaphore,
            last_api_call,
        }
    }

    pub async fn enqueue<F, Fut>(
        &self,
        f: F,
        rate_limited: bool,
    ) -> Result<oneshot::Receiver<Result<Fut::Output, TaskError>>, TaskError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
    {
        //let (job, rx) = Job::new(f);
        let (tx, rx) = oneshot::channel::<Result<Fut::Output, TaskError>>();
        let api_semaphore = self.api_semaphore.clone();
        let last_api_call = self.last_api_call.clone();

        let job: BoxedJob = Box::pin(async move {
            let mut permit: Option<SemaphorePermit> = None;
            if rate_limited {
                permit = Some(api_semaphore.acquire().await.unwrap());
            }

            if rate_limited {
                let last_time = last_api_call.lock().await;
                let elapsed = last_time.elapsed();
                drop(last_time);

                let d: u64 = 200;
                let wait_duration = if elapsed < Duration::from_millis(d) {
                    Duration::from_millis(d) - elapsed
                } else {
                    Duration::ZERO
                };

                if wait_duration > Duration::ZERO {
                    tokio::time::sleep(wait_duration).await;
                }

                let mut last_time: tokio::sync::MutexGuard<'_, Instant> =
                    last_api_call.lock().await;
                *last_time = Instant::now();
                drop(last_time);
            }

            match AssertUnwindSafe(f()).catch_unwind().await {
                Ok(output) => {
                    let _ = tx.send(Ok(output));
                }
                Err(join_err) => {
                    log_error(format!("Task panicked: {join_err:?}")).await;
                }
            }
        });

        self.sender
            .send(job)
            .await
            .map_err(|_| TaskError::DeserializationError("bob".to_string()))?;

        Ok(rx)
    }
}

/// Trait to normalize job input into `Result<T, TaskError>`
pub trait JobInput<T> {
    fn into_input(self) -> Result<T, TaskError>;
}

// Plain value
impl<T> JobInput<T> for T {
    fn into_input(self) -> Result<T, TaskError> {
        Ok(self)
    }
}

// Result<T, E>
impl<T, E> JobInput<T> for Result<T, E>
where
    E: Into<TaskError>,
{
    fn into_input(self) -> Result<T, TaskError> {
        self.map_err(Into::into)
    }
}

// Option<T>
impl<T> JobInput<T> for Option<T> {
    fn into_input(self) -> Result<T, TaskError> {
        self.ok_or(TaskError::Other("Option was None".into()))
    }
}

/// Trait to normalize job output into `Result<R, TaskError>`
pub trait JobOutput<R> {
    fn into_output(self) -> Result<R, TaskError>;
}

// Plain value
impl<R> JobOutput<R> for R {
    fn into_output(self) -> Result<R, TaskError> {
        Ok(self)
    }
}

// Result<R, E>
impl<R, E> JobOutput<R> for Result<R, E>
where
    E: Into<TaskError>,
{
    fn into_output(self) -> Result<R, TaskError> {
        self.map_err(Into::into)
    }
}

// Option<R>
impl<R> JobOutput<R> for Option<R> {
    fn into_output(self) -> Result<R, TaskError> {
        self.ok_or(TaskError::Other("Option was None".into()))
    }
}

pub async fn job<T, R, F, Fut, Out, In>(
    queue: &ProcessingQueue,
    input: oneshot::Receiver<Result<In, TaskError>>,
    rate_limited: bool,
    f: F,
) -> Option<oneshot::Receiver<Result<R, TaskError>>>
where
    In: JobInput<T> + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
    F: FnOnce(T) -> Fut + Send + 'static,
    Fut: Future<Output = Out> + Send + 'static,
    Out: JobOutput<R> + Send + 'static,
{
    match input.await {
        Ok(Ok(raw)) => match raw.into_input() {
            Ok(value) => { 
                let (tx, rx) = oneshot::channel();
                let fut = f(value);

                let enqueue_res = queue
                    .enqueue(
                        move || async move {
                            let result = fut.await.into_output();
                            let _ = tx.send(result);
                        },
                        rate_limited,
                    ).await;
                    
                    match enqueue_res {
                        Ok(_) => Some(rx),
                        Err(_) => None,
                    }
            },
            Err(e) => {
                let (tx, rx) = oneshot::channel();
                let _ = tx.send(Err(e));
                Some(rx)
            }
        },
        Ok(Err(e)) => {
            let (tx, rx) = oneshot::channel();
            let _ = tx.send(Err(e));
            Some(rx)
        }
        Err(_) => None,
    }
}

async fn print_map(map: HashMap<String, String>) {
    println!("aaaaaa");
    for (key, value) in map {
        println!("{key} {value}");
    }
}


async fn add_to_map(mut map:HashMap<String, String>) {
    map.insert("AAAAA".to_string(), "BBBBBBB".to_string());
}

async fn get_map() -> Result<HashMap<String, String>, TaskError> {
    println!("bbbb");
    let mut a = HashMap::new();
    a.insert("bob".to_string(), "1".to_string());
    a.insert("bob2".to_string(), "2".to_string());
    a.insert("bob3".to_string(), "3".to_string());
    Ok(a)
}

pub async fn do_stuff() -> Result<(), TaskError> {
    let queue = ProcessingQueue::new(100);


    let rec: oneshot::Receiver<Result<Result<HashMap<String, String>, TaskError>, TaskError>> = queue.enqueue(get_map, true).await?;
    let a  = job(&queue, rec, true, add_to_map).await.unwrap();
    let b  = job(&queue, a, true, |data| async move {print_map(data)}.await).await.unwrap();

    let r =  queue.enqueue(fetch_all_wvw_guild_ids, true).await?;
    let a: Result<Result<(), TaskError>, oneshot::error::RecvError> = job(&queue, r, true, print_map).await.unwrap().await;

       

    println!("end");
    Ok(())
}


//TODO: Errors are weird with the Q, Result<Result> boxing is weird ,try and make it more dynamic. so 
//Goal is just neat Firstjob().NextJeb().Nextjob()
//rewrite then() myself. then(closure that converts last outupt to new input, next async function)