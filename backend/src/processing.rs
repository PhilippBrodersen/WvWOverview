use std::{clone, error::Error, fmt};

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::mpsc};
use std::fmt::Debug;

use crate::database::add_guild;

#[derive(Debug)]
pub enum ProcessingError {
    DeserializationError(String),
    DatabaseError(sqlx::Error)
}

impl fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessingError::DeserializationError(msg) => write!(f, "Not Found Error: {}", msg),
            ProcessingError::DatabaseError(err) => write!(f, "IO Error: {}", err),
        }
    }
}

impl Error for ProcessingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ProcessingError::DatabaseError(err) => Some(err),
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
        serde_json::from_value(value).map_err(|e| {
            log_error(e)
        }).ok()
       
    }
}


async fn log_error<E: Debug>(err: E) {
    let debug_str = format!("{:?}\n", err);

    if let Ok(mut file) = OpenOptions::new() 
        .create(true)
        .append(true)
        .open("error.log")
        .await
    {
        if let Err(e) = file.write_all(debug_str.as_bytes()).await {
            eprintln!("Failed to write to log file: {:?}", e);
        }
    } else {
        eprintln!("Failed to open error.log");
    }
}



#[derive(serde::Deserialize, Debug, FromRow, Clone)]
pub struct Guild {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) tag: String,
    pub(crate) normalized_first_letter: String
}






pub enum DataProcessing {
    Deserialize(Deserialize),
    SaveToDB(SaveToDB)
}

pub enum Deserialize {
    Guild(Value)
}

pub enum SaveToDB {
    Guild(Guild)
}

impl SaveToDB {
    pub async fn save_to_db(self) {
        let result = match self {
            SaveToDB::Guild(guild) => add_guild(guild).await,
        };

        //do something with the error
    }
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
                //let tx_clone = tx.clone();
                tokio::spawn(async move {

                    match item {
                        DataProcessing::Deserialize(deserialize) => {
                            match deserialize {
                                Deserialize::Guild(value) => {
                                    if let Some(guild) = Guild::from_value(value) {
                                        tx_clone.send(DataProcessing::SaveToDB(SaveToDB::Guild(guild)));
                                    }
                                },
                            };
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

    pub async fn enqueue(&self, item: DataProcessing) {
        let _ = self.sender.send(item).await;
    }
}
