use serde::de::DeserializeOwned;
use serde_json::Value;
use sqlx::FromRow;
use std::fmt::Debug;
use std::{error::Error, fmt};
use tokio::sync::oneshot;
use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
    sync::mpsc::{self, Sender},
};

use crate::database::add_guild;

#[derive(Debug)]
pub enum TaskError {
    DeserializationError(String),
    DatabaseError(sqlx::Error),
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::DeserializationError(msg) => write!(f, "Not Found Error: {msg}"),
            TaskError::DatabaseError(err) => write!(f, "IO Error: {err}"),
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

pub trait ErrorHandler {
    fn handle_error<E: Debug>(error: E);
}

// Default behavior: just log to stderr
pub fn default_handle_error<T: ?Sized, E: Debug>(error: E) {
    eprintln!("Error for {}: {:?}", std::any::type_name::<T>(), error);
}

//maybe just a big handle_error function with a big match that just figures the logging out?

pub trait FromJsonValue: Sized {
    fn from_value(value: Value) -> Option<Self>;
}

// Blanket implementation for anything that implements DeserializeOwned
impl<T> FromJsonValue for T
where
    T: DeserializeOwned,
{
    fn from_value(value: Value) -> Option<Self> {
        serde_json::from_value(value).map_err(log_error).ok()
    }
}

async fn log_error<E: Debug>(err: E) {
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

trait deserializeable {}

#[derive(serde::Deserialize, Debug, FromRow, Clone)]
pub struct Guild {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) tag: String,
    pub(crate) normalized_first_letter: String,
}

impl deserializeable for Guild {}

pub enum DataProcessing {
    Deserialize(Deserialize),
    SaveToDB(SaveToDB),
}

pub enum Deserialize {
    Guild(Value),
}

pub enum SaveToDB {
    Guild(Guild),
}

pub enum Task {
    Deserialize { json: serde_json::Value },
    SaveGuild { guild: Guild },
}

pub struct Job<R> {
    pub task: Task,
    pub respond_to: oneshot::Sender<Result<R, TaskError>>,
}

pub struct ProcessingQueue {
    sender: mpsc::Sender<DataProcessing>,
}

impl ProcessingQueue {
    pub fn new(buffer: usize) -> Self {
        let (tx, mut rx) = mpsc::channel::<DataProcessing>(buffer);
        let tx_clone_for_task = tx.clone();
        tokio::spawn(async move {
            while let Some(item) = rx.recv().await {
                let tx_clone = tx_clone_for_task.clone();
                tokio::spawn(async move {
                    match item {
                        DataProcessing::Deserialize(deserialize) => match deserialize {
                            Deserialize::Guild(value) => {
                                if let Some(guild) = Guild::from_value(value) {
                                    Self::_enqueue(
                                        tx_clone,
                                        DataProcessing::SaveToDB(SaveToDB::Guild(guild)),
                                    );
                                }
                            }
                        },
                        DataProcessing::SaveToDB(save_to_db) => {
                            match save_to_db {
                                SaveToDB::Guild(guild) => add_guild(guild).await,
                            };
                        }
                    }
                });
            }
        });

        Self { sender: tx }
    }

    async fn _enqueue(sender: Sender<DataProcessing>, item: DataProcessing) {
        if let Err(err) = sender.send(item).await {
            log_error(err);
        }
    }

    pub async fn enqueue(&self, item: DataProcessing) {
        Self::_enqueue(self.sender.clone(), item);
    }
}
