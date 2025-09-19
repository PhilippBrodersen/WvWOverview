use std::{collections::{BTreeMap, HashMap}, fmt, fs::OpenOptions, io::Write, sync::Arc,};

use chrono::{Duration, Utc};
use futures::{StreamExt, stream::FuturesUnordered};
use phf::phf_map;
use sqlx::SqlitePool;
use tokio::{fs, io, sync::RwLock, time::{self}};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::{
    data::{Data, Match, MatchColor, MatchData, Tier},
    database::{add_guild, get_all_guild_teams, get_all_matches, get_guilds_for_team, get_last_guild_update, get_match, get_team_id_for_guild, guild_exists, upsert_guild_team, upsert_guild_team_null, upsert_match},
    gw2api::{fetch_all_wvw_guild_ids, fetch_guild_info, fetch_match},
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

fn log_error<E: fmt::Debug>(err: E) {
    let debug_str = format!("{err:?}\n");

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("error.log")
    {
        if let Err(e) = file.write_all(debug_str.as_bytes()) {
            eprintln!("Failed to write to log file: {e:?}");
        }
    } else {
        eprintln!("Failed to open error.log");
    }
}

pub async fn run_match_updater(pool: &SqlitePool) {
    let mut interval = time::interval(tokio::time::Duration::from_secs(60));

    let pool = pool.clone();
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            update_matches(&pool).await;
        }
    });
}

pub async fn update_matches(pool: &SqlitePool) {
    let mut tasks = FuturesUnordered::new();
    println!("updating matches");
    for tier in Tier::all() {
        let pool: sqlx::Pool<sqlx::Sqlite> = pool.clone();
        tasks.push(tokio::spawn(async move {
            match fetch_match(tier).await {
                Ok(m) => {
                    if let Err(err) = upsert_match(&pool, &m).await {
                        println!("AAAAAA");
                        log_error(err);
                    }
                }
                Err(err) => {
                    println!("BBBBB");
                    log_error(err);
                }
            }
        }));
    }

    while tasks.next().await.is_some() {}
}

pub async fn run_guild_updater(pool: &SqlitePool) {
    let mut interval = time::interval(tokio::time::Duration::from_secs(60));

    let pool = pool.clone();
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            update_guilds(&pool).await;
        }
    });
}

pub async fn update_guilds(pool: &SqlitePool) {
    let mut tasks = FuturesUnordered::new();
    println!("HI");

    let result: HashMap<String, String> = match fetch_all_wvw_guild_ids().await {
        Ok(ids) => ids,
        Err(err) => {
            log_error(err);
            return;
        }
    };
    let guild_ids: Vec<String> = result.keys().cloned().collect();

    println!("Looping over ids");

    for (guild_id, team_id) in result.clone() {
        let pool: sqlx::Pool<sqlx::Sqlite> = pool.clone();
        tasks.push(tokio::spawn(async move {
            let exists = guild_exists(&pool, &guild_id).await.unwrap_or_else(|err| {
                println!("1");
                log_error(err);
                false
            });
            let last_update = get_last_guild_update(&pool, &guild_id)
                .await
                .unwrap_or_else(|err| {
                    println!("2");
                    log_error(err);
                    None
                });

            if !exists || last_update.is_none_or(|ts| Utc::now() - ts > Duration::hours(24)) {

                match fetch_guild_info(&guild_id).await {
                    Ok(Some(guild)) => {
                        match add_guild(&pool, guild).await {
                            Ok(_) => {
                                 if let Err(err) = upsert_guild_team(&pool, &guild_id, Some(&team_id)).await {
                                    println!("3");
                                    log_error(err);
                                }
                            },
                            Err(err) => {
                                println!("4");
                                log_error(err);        
                            },
                        }
                    }
                    Ok(None) => { },
                    Err(err) => {log_error(err); println!("5")},
                }
            }

            //let team_id: Option<&str> = if team_id.is_empty() { None } else { Some(&team_id) };
           
        }));
    }

    while tasks.next().await.is_some() {}

    let exclude: Vec<String> = result.keys().cloned().collect();
    if let Err(err) = upsert_guild_team_null(&pool, exclude).await {
        log_error(err);
    }
}

fn normalize_name(name: &str) -> String {
    name.trim().nfd()
        .filter(|c| c.is_ascii() || !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

fn group_guilds(guilds: Vec<String>) -> BTreeMap<char, Vec<String>> {
    let mut grouped: BTreeMap<char, Vec<String>> = BTreeMap::new();
    for g in guilds {
        let first = normalize_name(&g).chars().next().unwrap_or('#').to_ascii_uppercase();
        grouped.entry(first).or_default().push(g.clone());
    }
    // sort each group
    for v in grouped.values_mut() {
        v.sort_by_key(|name| normalize_name(name));
    }
    grouped
}

fn fix_team_ids(s: &str) -> String {
    let a = if s.len() == 4 { format!("1{}", s) } else { s.to_string() };
    if a == "12101" {"12015".to_owned()} else {a}
}


pub async fn run_mateches_cache_updater(pool: &SqlitePool, cache: Arc<RwLock<Data>>,) {
    let mut interval: time::Interval = time::interval(tokio::time::Duration::from_secs(1));

    let pool = pool.clone();
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            let data = build_data(&pool).await;
            //make this available to the endpoint in a cached way

            let mut write_guard = cache.write().await;
            *write_guard = data;
        }
    });
}

async fn read_lines_into_vec(filename: &str) -> Vec<String> {
    match fs::read_to_string(filename).await {
        Ok(content) => {
           content
            .lines() 
            .map(|s| s.to_string())
            .collect()           
        },
        Err(err) => { log_error(err); vec![]},
    }
}

const IMPORTANT_GUILDS: &str = include_str!("../static/important_guilds.txt");

pub async fn build_data(pool: &SqlitePool) -> Data {
    let team_id = get_team_id_for_guild(&pool, "Quality Ôver Quantity").await.unwrap_or(Some("0".to_string())).unwrap_or("0".to_string());
    Data { 
        matches: build_all_matches(&pool).await,
        important_guilds: IMPORTANT_GUILDS.lines().map(|l| l.to_string()).collect(),
        our_team: TEAM_NAMES.get(&team_id).map_or(format!("Unknown"), |name| name.to_string()),
    }
}


pub async fn build_all_matches(pool: &SqlitePool

) -> BTreeMap<u8, MatchData> {
    let mut all_matches = BTreeMap::new();

    let tiers = Tier::all();

    for i in 0..5{
        let tier = &tiers[i];
        if let Some(m) = get_match(&pool, *tier).await.unwrap() {
            let t_id_red = fix_team_ids(&m.worlds.red.to_string());
            let t_id_green = fix_team_ids(&m.worlds.green.to_string());
            let t_id_blue = fix_team_ids(&m.worlds.blue.to_string());
                

            let red: MatchColor = MatchColor {
                team_name: TEAM_NAMES.get(&t_id_red).map_or(format!("Red-{i}"), |name| name.to_string()),
                victory_points: m.victory_points.red.to_string(),
                guilds: group_guilds( get_guilds_for_team(&pool, &t_id_red).await.unwrap_or_default().iter().map(|g| g.to_string()).collect())
            };

            let green = MatchColor {
                team_name: TEAM_NAMES.get(&t_id_green).map_or(format!("Green-{i}"), |name| name.to_string()),
                victory_points: m.victory_points.green.to_string(),
                guilds: group_guilds( get_guilds_for_team(&pool, &t_id_green).await.unwrap_or_default().iter().map(|g| g.to_string()).collect())
            };

            let blue = MatchColor {
                team_name: TEAM_NAMES.get(&t_id_blue).map_or(format!("Blue-{i}"), |name| name.to_string()),
                victory_points: m.victory_points.blue.to_string(),
                guilds: group_guilds( get_guilds_for_team(&pool, &t_id_blue).await.unwrap_or_default().iter().map(|g| g.to_string()).collect())
            };

            let m = MatchData {
                red,
                green,
                blue,
            };

            all_matches.insert(i as u8, m);
        }
    }
    all_matches 
}


