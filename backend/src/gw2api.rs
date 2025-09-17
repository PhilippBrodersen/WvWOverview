use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::sleep;

static API_BASE: &str = "https://api.guildwars2.com/v2";

#[derive(Debug, Clone)]
pub enum APIEndPoint {
    Guild(String),
    AllGuildIDs,
}

impl ToString for APIEndPoint {
    fn to_string(&self) -> String {
        match self {
            APIEndPoint::Guild(id) => format!("{API_BASE}/guild/{id}"),
            APIEndPoint::AllGuildIDs => format!("{API_BASE}/wvw/guilds/eu"),
        }
    }
}

pub struct APIQueue {
    sender: mpsc::Sender<APIEndPoint>,
}

pub enum ProcessItem {
    GuildInfo,
    // Add more variants as needed
}

impl APIQueue {
    pub fn new(buffer: usize, delay_ms: u64, proc_tx: mpsc::Sender<ProcessItem>) -> Self {
        let (tx, mut rx) = mpsc::channel::<APIEndPoint>(buffer);
        let proc_tx = Arc::new(proc_tx);

        tokio::spawn({
            let proc_tx = Arc::clone(&proc_tx);
            async move {
                let delay = Duration::from_millis(delay_ms);
                while let Some(endpoint) = rx.recv().await {
                    match reqwest::get(endpoint.to_string()).await {
                        Ok(resp) => match resp.json::<Value>().await {
                            Ok(json) => {
                                // Send to processing queue
                                /* let _ = proc_tx
                                    .send(ProcessItem::GuildInfo {
                                        guild_id: req.url.clone(),
                                        json,
                                    })
                                    .await; */
                            }
                            Err(e) => eprintln!("Failed to parse JSON {}: {}", e, e),
                        },
                        Err(e) => eprintln!("Failed to fetch {}: {}", e, e),
                    }

                    // Rate limit
                    tokio::time::sleep(delay).await;
                }
            }
        });

        Self { sender: tx }
    }

    /* pub async fn enqueue(&self, req: FetchRequest) {
        let _ = self.sender.send(req).await;
    } */
}


enum Data {
    Guild(Guild),
    Team(Team),
    Match(Match)

}

/* enum ApiCall {
    AllGuilds,
    GuildInfo(String), //String = guild_id
}

pub struct APIQueue {
    sender: mpsc::Sender<ApiCall>,
}

//make api queue more raw: just url -> json with timer for mox speed

impl APIQueue {
    pub fn new(buffer: usize, delay_ms: u64) -> Self {
        let (tx, mut rx) = mpsc::channel::<ApiCall>(buffer);

        // Spawn the worker
         let tx_copy = tx.clone();
        tokio::spawn(async move {
            while let Some(item) = rx.recv().await {

                match item {
                    ApiCall::AllGuilds => {
                        let ids= fetch_all_wvw_guild_ids().await.unwrap();
                       
                        for (guild_id, team_id) in ids {
                            tx_copy.send(ApiCall::GuildInfo(guild_id)).await.unwrap();
                        }
                    }
                    ApiCall::GuildInfo(id) => {
                        let guild = fetch_guild_info(&id).await.unwrap();
                        println!("Processing: {:?}", guild);
                    }
                }

                // Rate limiting between items
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        });

        Self { sender: tx }
    }

    /// Enqueue an item
    pub async fn enqueue(&self, item: ApiCall) {
        let _ = self.sender.send(item).await;
    }
} */

enum Tier {
    One,
    Two,
    Three,
    Four,
    Five,
}

impl Tier {
    fn as_str(&self) -> &str {
        match self {
            Tier::One => "1",
            Tier::Two => "2",
            Tier::Three => "3",
            Tier::Four => "4",
            Tier::Five => "5",
        }
    }
}

// Enum for the three team colors
#[derive(Debug, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")] // maps "red" -> Team::Red
enum TeamColor {
    Red,
    Green,
    Blue,
}

// Struct for just the fields you care about
#[derive(Debug, Deserialize)]
struct Match {
    worlds: HashMap<TeamColor, String>,
    victory_points: HashMap<TeamColor, String>,
}

#[derive(serde::Deserialize, Debug)]
struct Guild {
    id: String,
    name: String,
    tag: String,
}

struct Team {
    id: String,
    guilds: Vec<Guild>,
    score: String
}

pub async fn fetch_all_wvw_guild_ids() -> Result<HashMap<String, String>, reqwest::Error> {
    let url = &format!("{}/wvw/guilds/eu", API_BASE);

    let map: HashMap<String, String> = reqwest::get(url)
        .await?
        .json::<HashMap<String, String>>()
        .await?;
    Ok(map)
}

pub async fn fetch_guild_info(guild_id: &str) -> Result<Guild, reqwest::Error> {
    let url = &format!("{}/guild/{}", API_BASE, guild_id);

    let guild: Guild = reqwest::get(url)
        .await?
        .json::<Guild>()
        .await?;
    Ok(guild)
}

pub async fn fetch_match(tier: Tier) -> Result<Match, reqwest::Error> {
    let url = &format!("{}/wvw/matches/2{}", API_BASE, tier.as_str());

    let m: Match = reqwest::get(url)
        .await?
        .json::<Match>()
        .await?;
    Ok(m)
}