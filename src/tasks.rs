#![warn(clippy::pedantic)]

use std::{
    collections::{BTreeMap, HashMap},
    env::{self},
    fmt,
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::Arc,
};

use futures::{StreamExt, stream::FuturesUnordered};
use phf::phf_map;
use sqlx::SqlitePool;
use tokio::{
    sync::RwLock,
    time::{self},
};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::{
    data::{APIEndpoint, Data, Guild, Match, MatchColor, MatchData, Tier},
    database::{
        get_guilds_for_team, get_match, get_team_id_for_guild, guild_in_db, guilds_to_update,
        upsert_guild, upsert_guild_team_null, upsert_guild_teams_bulk, upsert_match,
    },
    rate_limiter::{ApiQueue, Priority},
};

static TEAM_NAMES: phf::Map<&'static str, &'static str> = phf_map! {
    "11001" => "Moogooloo",
    "11002" => "Rall's Rest",
    "11003" => "Domain of Torment",
    "11004" => "Yohlon Haven",
    "11005" => "Tombs of Drascir",
    "11006" => "Hall of Judgment",
    "11007" => "Throne of Balthazar",
    "11008" => "Dwayna's Temple",
    "11009" => "Abaddon's Prison",
    "11010" => "Cathedral of Blood",
    "11011" => "Lutgardis Conservatory",
    "11012" => "Mosswood",
    "12001" => "Skrittsburgh",
    "12002" => "Fortune's Vale",
    "12003" => "Silent Woods",
    "12004" => "Ettin's Back",
    "12005" => "Domain of Anguish",
    "12006" => "Palawadan",
    "12007" => "Bloodstone Gulch",
    "12008" => "Frost Citadel",
    "12009" => "Dragrimmar",
    "12010" => "Grenth's Door",
    "12011" => "Mirror of Lyssa",
    "12012" => "Melandru's Dome",
    "12013" => "Kormir's Library",
    "12014" => "Great House Aviary",
    "12015" => "Bava Nisos",
};

pub fn log_error<E: fmt::Debug>(err: E) {
    let default_file = "error.log";

    let error_path: PathBuf = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(default_file)))
        .unwrap_or_else(|| PathBuf::from(default_file));

    let debug_str = format!("{:?}: {err:?}\n", time::Instant::now());

    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&error_path)
    {
        Ok(mut file) => {
            if let Err(e) = file.write_all(debug_str.as_bytes()) {
                eprintln!(
                    "Failed to write to log file {}: {e:?}",
                    error_path.display()
                );
            }
        }
        Err(e) => {
            eprintln!("Failed to open log file {}: {e:?}", error_path.display());
        }
    }
}

pub fn start_update_loops(pool: &SqlitePool, api_queue: &Arc<ApiQueue>) {
    tokio::spawn({
        let pool = pool.clone();
        let api_queue = api_queue.clone();
        async move { update_matches(&pool, api_queue).await }
    });

    tokio::spawn({
        let pool = pool.clone();
        let api_queue = api_queue.clone();
        async move { update_teams(&pool, api_queue).await }
    });

    tokio::spawn({
        let pool = pool.clone();
        let api_queue = api_queue.clone();
        async move { update_known_guilds(&pool, api_queue).await }
    });
}

pub async fn update_matches(pool: &SqlitePool, api_queue: Arc<ApiQueue>) {
    let mut interval = time::interval(tokio::time::Duration::from_secs(60));

    loop {
        interval.tick().await;

        let mut tasks = FuturesUnordered::new();

        for tier in Tier::all() {
            let pool = pool.clone();
            let api_queue = api_queue.clone();

            tasks.push(async move {
                if let Some(m) = api_queue
                    .enqueue::<Match>(&APIEndpoint::Match(tier), Priority::High)
                    .await
                {
                    upsert_match(&pool, &m).await;
                }
            });
        }

        while tasks.next().await.is_some() {}
    }
}

fn sort_guilds(
    unsorted_guilds: HashMap<String, String>,
    my_guild_id: &str,
) -> Vec<(String, String)> {
    let mut my_guild_group: Vec<(String, String)> = Vec::new();
    let mut my_team_group = Vec::new();
    let mut other_guilds = Vec::new();

    if let Some(my_team_id) = unsorted_guilds.get(my_guild_id) {
        for (guild_id, team_id) in &unsorted_guilds {
            let guild_id = guild_id.clone();
            let team_id = team_id.clone();

            if guild_id == my_guild_id {
                my_guild_group.push((guild_id, team_id));
            } else if team_id == *my_team_id {
                my_team_group.push((guild_id, team_id));
            } else {
                other_guilds.push((guild_id, team_id));
            }
        }
    } else {
        other_guilds = unsorted_guilds.into_iter().collect();
    }

    my_guild_group.extend(my_team_group);
    my_guild_group.extend(other_guilds);
    my_guild_group
}

pub async fn update_known_guilds(pool: &SqlitePool, api_queue: Arc<ApiQueue>) {
    let mut interval = time::interval(tokio::time::Duration::from_secs(60));

    loop {
        interval.tick().await;

        let guild_ids = guilds_to_update(pool).await;

        let mut tasks = FuturesUnordered::new();

        for id in guild_ids {
            tasks.push(async {
                if let Some(guild) = api_queue
                    .enqueue::<Guild>(&APIEndpoint::Guild(id), Priority::Low)
                    .await
                {
                    upsert_guild(pool, guild).await;
                }
            });
        }

        while tasks.next().await.is_some() {}
    }
}

pub async fn update_teams(pool: &SqlitePool, api_queue: Arc<ApiQueue>) {
    let mut interval = time::interval(tokio::time::Duration::from_secs(60));

    loop {
        interval.tick().await;

        if let Some(guild_map) = api_queue
            .enqueue::<HashMap<String, String>>(&APIEndpoint::AllWvWGuilds, Priority::High)
            .await
        {
            upsert_guild_team_null(pool, guild_map.keys().cloned().collect()).await;

            let guild_list = match api_queue
                .enqueue::<[String; 1]>(
                    &APIEndpoint::GuildIDfromName("Quality Ôver Quantity".to_string()),
                    Priority::High,
                )
                .await
                .map(|[s]| s)
            {
                Some(id) => sort_guilds(guild_map, &id),
                None => guild_map.into_iter().collect(),
            };

            upsert_guild_teams_bulk(pool, guild_list.clone()).await;

            let mut tasks = FuturesUnordered::new();
            for (guild_id, _) in guild_list {
                let pool = pool.clone();
                let api_queue = api_queue.clone();

                tasks.push(async move {
                    let exists: bool = guild_in_db(&pool, &guild_id).await;

                    if !exists
                        && let Some(guild) = api_queue
                            .enqueue::<Guild>(&APIEndpoint::Guild(guild_id), Priority::Normal)
                            .await
                    {
                        upsert_guild(&pool, guild).await;
                    }
                });
            }

            while tasks.next().await.is_some() {}
        }
    }
}

fn normalize_name(name: &str) -> String {
    name.trim()
        .nfd()
        .filter(|c| c.is_ascii() || !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

fn group_guilds(guilds: Vec<String>) -> BTreeMap<char, Vec<String>> {
    let mut grouped: BTreeMap<char, Vec<String>> = BTreeMap::new();
    for g in guilds {
        let first = normalize_name(&g)
            .chars()
            .next()
            .unwrap_or('#')
            .to_ascii_uppercase();
        grouped.entry(first).or_default().push(g.clone());
    }

    for v in grouped.values_mut() {
        v.sort_by_key(|name| normalize_name(name));
    }
    grouped
}

fn fix_team_ids(s: &str) -> String {
    let a = if s.len() == 4 {
        format!("1{s}")
    } else {
        s.to_string()
    };
    if a == "12101" { "12015".to_owned() } else { a }
}

pub async fn run_mateches_cache_updater(pool: &SqlitePool, cache: Arc<RwLock<Data>>) {
    let mut interval: time::Interval = time::interval(tokio::time::Duration::from_secs(1));

    let pool = pool.clone();
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            let data = build_data(&pool).await;

            let mut write_guard = cache.write().await;
            *write_guard = data;
        }
    });
}

const IMPORTANT_GUILDS: &str = include_str!("../static/important_guilds.txt");

pub async fn build_data(pool: &SqlitePool) -> Data {
    let team_id: String = get_team_id_for_guild(pool, "Unga Bunga On Eh Bu Ga")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "0".to_string());
    Data {
        matches: build_all_matches(pool).await,
        important_guilds: IMPORTANT_GUILDS
            .lines()
            .map(std::string::ToString::to_string)
            .collect(),
        our_team: TEAM_NAMES
            .get(&team_id)
            .map_or(String::new(), |name| (*name).to_string()),
    }
}

pub async fn build_all_matches(pool: &SqlitePool) -> BTreeMap<usize, MatchData> {
    let mut all_matches = BTreeMap::new();

    for (i, tier) in Tier::all().into_iter().enumerate() {
        if let Some(m) = get_match(pool, tier).await {
            let ids = [
                fix_team_ids(&m.worlds.red.to_string()),
                fix_team_ids(&m.worlds.green.to_string()),
                fix_team_ids(&m.worlds.blue.to_string()),
            ];

            let vp = [
                m.victory_points.red,
                m.victory_points.green,
                m.victory_points.blue,
            ];

            let mut team = vec![];

            for i in 0..3 {
                let t = MatchColor {
                    team_name: TEAM_NAMES
                        .get(&ids[i])
                        .map_or_else(|| "Unknown".to_string(), |name| (*name).to_string()),
                    victory_points: vp[i].to_string(),
                    guilds: group_guilds(
                        get_guilds_for_team(pool, &ids[i])
                            .await
                            .unwrap_or_default()
                            .iter()
                            .map(std::string::ToString::to_string)
                            .collect(),
                    ),
                };

                team.push(t);
            }

            let m = MatchData {
                red: team[0].clone(),
                green: team[1].clone(),
                blue: team[2].clone(),
            };

            all_matches.insert(i, m);
        }
    }
    all_matches
}
