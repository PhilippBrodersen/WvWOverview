import sqlite3
import datetime
from pathlib import Path
from typing import Optional, Any, Dict
import aiosqlite
from contextlib import asynccontextmanager
import unicodedata
from collections import defaultdict

teams = [
    ("11001", None, "Moogooloo"),
    ("11002", None, "Rall's Rest"),
    ("11003", None, "Domain of Torment"),
    ("11004", None, "Yohlon Haven"),
    ("11005", None, "Tombs of Drascir"),
    ("11006", None, "Hall of Judgment"),
    ("11007", None, "Throne of Balthazar"),
    ("11008", None, "Dwayna's Temple"),
    ("11009", None, "Abaddon's Prison"),
    ("11010", None, "Cathedral of Blood"),
    ("11011", None, "Lutgardis Conservatory"),
    ("11012", None, "Mosswood"),
    ("12001", None, "Skrittsburgh"),
    ("12002", None, "Fortune's Vale"),
    ("12003", None, "Silent Woods"),
    ("12004", None, "Ettin's Back"),
    ("12005", None, "Domain of Anguish"),
    ("12006", None, "Palawadan"),
    ("12007", None, "Bloodstone Gulch"),
    ("12008", None, "Frost Citadel"),
    ("12009", None, "Dragrimmar"),
    ("12010", None, "Grenth's Door"),
    ("12011", None, "Mirror of Lyssa"),
    ("12012", None, "Melandru's Dome"),
    ("12013", None, "Kormir's Library"),
    ("12014", None, "Great House Aviary"),
    ("12015", "12101", "Bava Nisos"),  # Alt ID directly in data
]

def normalize_letter(name: str) -> str:
    name = name.upper()
    nfkd_form = unicodedata.normalize('NFKD', name)
    normalized = ''.join([c for c in nfkd_form if not unicodedata.combining(c)])
    return normalized[0] if normalized else "#"

DB_PATH = Path(__file__).parent / "app.db"


@asynccontextmanager
async def get_async_connection():
    """Get an async SQLite connection with WAL enabled, reusable in async with."""
    db = await aiosqlite.connect(DB_PATH)
    db.row_factory = aiosqlite.Row  # return dict-like rows
    await db.execute("PRAGMA journal_mode=WAL;")
    try:
        yield db
    finally:
        await db.close()


async def init_db():
    """Create tables if they don't exist."""
    async with get_async_connection() as db:
        cur = await db.cursor()

        # Teams
        await cur.execute("""
        CREATE TABLE IF NOT EXISTS teams (
            id TEXT PRIMARY KEY,       -- teamid is a hex string
            alt_id TEXT UNIQUE,
            name TEXT NOT NULL
        );
        """)

        # Guilds
        await cur.execute("""
        CREATE TABLE IF NOT EXISTS guilds (
            id TEXT PRIMARY KEY,       -- guildid is a hex string
            name TEXT NOT NULL,
            tag TEXT
            normalized_letter TEXT
        );
        """)

        await cur.execute("""
            CREATE TABLE IF NOT EXISTS guild_team (
            guild_id TEXT PRIMARY KEY,     -- each guild belongs to only one team
            team_id TEXT NOT NULL,
            FOREIGN KEY (guild_id) REFERENCES guilds(id) ON DELETE CASCADE,
            FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE CASCADE
        );
        """)

        # Matchups: each entry has 3 teams
        await cur.execute("""
            CREATE TABLE IF NOT EXISTS matchups (
            tier INTEGER NOT NULL CHECK (tier BETWEEN 1 AND 5),
            team_id TEXT NOT NULL,
            color TEXT NOT NULL CHECK (color IN ('red', 'blue', 'green')),
            score INTEGER DEFAULT 0,

            PRIMARY KEY (tier, color), -- 1 red, 1 blue, 1 green per tier
            FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE CASCADE
        );
        """)

        await cur.execute("""
        CREATE TABLE IF NOT EXISTS update_status (
            item_type TEXT NOT NULL,      -- 'guild', 'team', etc
            item_id TEXT NOT NULL,        -- guild_id, team_id, etc
            status TEXT NOT NULL DEFAULT 'pending',   -- 'pending', 'success', 'failed'
            last_attempt TIMESTAMP,
            retry_count INTEGER DEFAULT 0,
            PRIMARY KEY (item_type, item_id)
        );
        """)

        await cur.execute("""
            CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        """)

        await db.commit()
        await create_matchup_view()

async def create_matchup_view():
    """Create or replace the vw_matchup_hierarchy view."""
    async with get_async_connection() as db:
        cur = await db.cursor()
        await cur.execute("""
        CREATE VIEW IF NOT EXISTS vw_matchup_hierarchy AS
        SELECT
            m.tier,
            t.id AS team_id,
            t.name AS team_name,
            m.color AS team_color,
            m.score AS team_score,
            g.id AS guild_id,
            g.name AS guild_name,
            g.tag AS guild_tag,
            g.normalized_letter AS guild_letter
        FROM matchups m
        JOIN teams t ON t.id = m.team_id
        LEFT JOIN guild_team gt ON gt.team_id = t.id
        LEFT JOIN guilds g ON g.id = gt.guild_id
        ORDER BY 
            m.tier,
            m.color,
            t.name,
            g.normalized_letter,
            g.name;
        """)
        await db.commit()


#new stuff
async def get_team_for_guild_name(guild_name: str) -> Optional[Dict[str, Any]]:
    """
    Given a guild name, return the team it belongs to.
    Returns a dict like {'team_id': ..., 'team_name': ...} or None if not found.
    """
    async with get_async_connection() as db:
        cur = await db.cursor()
        await cur.execute("""
            SELECT t.id AS team_id, t.name AS team_name
            FROM guilds g
            JOIN guild_team gt ON g.id = gt.guild_id
            JOIN teams t ON gt.team_id = t.id
            WHERE g.name = ?
        """, (guild_name,))
        row = await cur.fetchone()
        return dict(row) if row else None

async def batch_execute(sql: str, values: list[tuple]):
    """
    Generic async batch insert/update helper.
    Executes all rows in a single transaction.
    """
    async with get_async_connection() as db:
        await db.executemany(sql, values)
        await db.commit()

async def set_new_guilds_pending(guild_ids: list[str]):
    """Mark a list of guilds as pending in update_status, only if not already present."""
    
    sql = """
    INSERT INTO update_status (item_type, item_id, status, last_attempt, retry_count)
    VALUES (?, ?, ?, ?, 0)
    ON CONFLICT(item_type, item_id) DO NOTHING
    """

    now = datetime.datetime.utcnow()
    # Prepare tuples: (item_type, item_id, status, last_attempt)
    values = [("guild", guild_id, "pending", now) for guild_id in guild_ids]

    await batch_execute(sql, values)

async def set_teams_for_guilds(guild_team: dict[str, str]):
    """Batch insert or ignore guilds into update_status."""
    sql = """
        INSERT INTO guild_team (guild_id, team_id)
        VALUES (?, ?)
        ON CONFLICT(guild_id) DO UPDATE SET team_id=excluded.team_id
    """
    values = [(k, v) for k, v in guild_team.items()]
    await batch_execute(sql, values)

async def set_matchup(tier: int, matchup: dict[str, dict[str, int]]):
    """Replace matchup data for a single tier (3 rows)."""
    values = [(tier, data["team_id"], color, data["score"]) 
              for color, data in matchup.items()]

    async with get_async_connection() as db:
        # Clear old rows for this tier
        await db.execute("DELETE FROM matchups WHERE tier = ?", (tier,))

        # Insert 3 fresh rows
        await db.executemany(
            "INSERT INTO matchups (tier, team_id, color, score) VALUES (?, ?, ?, ?)",
            values,
        )
        await db.commit()
        print("new matchup in db")

async def get_missing_guilds(guild_ids: list[str]) -> list[str]:
    async with get_async_connection() as db:
        cur = await db.execute("SELECT id FROM guilds")
        rows = await cur.fetchall()
        existing_ids = {row[0] for row in rows}
        return [gid for gid in guild_ids if gid not in existing_ids]

async def add_guild(guild_id: str, name: str, tag: str):
    """Insert or update a guild."""
    async with get_async_connection() as conn:
        normalized_letter = normalize_letter(name)
        await conn.execute(
            "INSERT OR IGNORE INTO guilds (id, name, tag, normalized_letter) VALUES (?, ?, ?, ?)",
            (guild_id, name, tag, normalized_letter),
        )
        await conn.commit()
        print("added to db")

async def get_all_guilds_for_team(team_id: str) -> list[dict]:
    """
    Returns all guilds belonging to the given team_id.
    Each guild is returned as a dict with keys: id, name, tag.
    """
    async with get_async_connection() as db:
        cur = await db.execute(
            """
            SELECT g.id, g.name, g.tag
            FROM guilds g
            JOIN guild_team gt ON g.id = gt.guild_id
            WHERE gt.team_id = ?
            """,
            (team_id,)
        )
        rows = await cur.fetchall()
        return [dict(row) for row in rows]

async def get_team_name(team_id: str) -> Optional[Dict[str, Any]]:
    """
    Get the team name by ID or alt_id.
    Tries to match id first, then alt_id.
    Returns None if not found.
    """
    async with get_async_connection() as db:
        cur = await db.cursor()
        await cur.execute(
            "SELECT name FROM teams WHERE id = ? OR alt_id = ?",
            (team_id, team_id)
        )
        row = await cur.fetchone()
        return row[0] if row else None

async def get_guild_info(guild_id: str) -> Optional[Dict[str, Any]]:
    async with get_async_connection() as db:
        cur = await db.execute("SELECT * FROM guilds WHERE id = ?", (guild_id,))
        row = await cur.fetchone()
        return row

# 1️⃣ Get all guild IDs belonging to a team
async def get_guilds_for_team(team_id: str) -> list[str]:
    async with get_async_connection() as db:
        cur = await db.execute(
            "SELECT guild_id FROM guild_team WHERE team_id = ?",
            (team_id,)
        )
        rows = await cur.fetchall()
        return [row[0] for row in rows]

# 2️⃣ Get the team ID a guild belongs to
async def get_team_for_guild(guild_id: str) -> Optional[str]:
    async with get_async_connection() as db:
        cur = await db.execute(
            "SELECT team_id FROM guild_team WHERE guild_id = ?",
            (guild_id,)
        )
        row = await cur.fetchone()
        return row[0] if row else None

async def get_all_matchups() -> list[dict]:
    """Return all matchups with tier, team_id, color, and score."""
    async with get_async_connection() as db:
        cur = await db.execute("SELECT tier, team_id, color, score FROM matchups ORDER BY tier, color")
        rows = await cur.fetchall()
        return [dict(row) for row in rows]

async def get_matchup_hierarchy():
    """Query the view and return structured dictionary by tier → team → guild letter."""
    async with get_async_connection() as db:
        cur = await db.cursor()
        await cur.execute("SELECT * FROM vw_matchup_hierarchy")
        rows = await cur.fetchall()

    tiers = defaultdict(list)

    for row in rows:
        tier = row["tier"]
        team_id = row["team_id"]

        # Find existing team in this tier
        team = next((t for t in tiers[tier] if t["team_id"] == team_id), None)
        if not team:
            team = {
                "team_id": team_id,
                "team_name": row["team_name"],
                "team_color": row["team_color"],
                "team_score": row["team_score"],
                "guilds": defaultdict(list)
            }
            tiers[tier].append(team)

        if row["guild_id"]:
            letter = row["guild_letter"] or "#"
            team["guilds"][letter].append({
                "guild_id": row["guild_id"],
                "guild_name": row["guild_name"],
                "guild_tag": row["guild_tag"]
            })

    return tiers

