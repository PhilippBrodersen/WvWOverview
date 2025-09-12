import sqlite3
import datetime
from pathlib import Path
from typing import Optional
import aiosqlite
from contextlib import asynccontextmanager

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
        
        await cur.execute("DELETE FROM teams")
        await cur.executemany(
            "INSERT INTO teams (id, alt_id, name) VALUES (?, ?, ?)",
            teams
        )
        await db.commit()

        # Guilds
        await cur.execute("""
        CREATE TABLE IF NOT EXISTS guilds (
            id TEXT PRIMARY KEY,       -- guildid is a hex string
            name TEXT NOT NULL,
            tag TEXT
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

#new stuff
SQL = {
    "guild_team_upsert": """
        INSERT INTO guild_team (guild_id, team_id)
        VALUES (?, ?)
        ON CONFLICT(guild_id) DO UPDATE SET team_id=excluded.team_id
    """,
    "update_status_insert_or_ignore": """
        INSERT OR IGNORE INTO update_status (item_type, item_id, status, last_attempt, retry_count)
        VALUES (?, ?, ?, ?, 0)
    """,
}

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
        await conn.execute(
            "INSERT OR IGNORE INTO guilds (id, name, tag) VALUES (?, ?, ?)",
            (guild_id, name, tag),
        )
        await conn.commit()
        print("added to db")

# ----------------------------
# STATUS OPERATIONS
# ----------------------------

async def set_status(item_type: str, item_id: str, status: str, allow_update: bool = True):
    """
    Insert or update a row in update_status.
    If allow_update is False, existing rows will NOT be updated.
    """
    async with get_async_connection() as conn:
        conn.execute("""
            INSERT INTO update_status (item_type, item_id, status, last_attempt, retry_count)
            VALUES (?, ?, ?, ?, 0)
            ON CONFLICT(item_type, item_id) DO UPDATE SET
                status = CASE WHEN ? THEN excluded.status ELSE update_status.status END,
                last_attempt = CASE WHEN ? THEN excluded.last_attempt ELSE update_status.last_attempt END,
                retry_count = CASE 
                    WHEN ? AND excluded.status='failed' THEN update_status.retry_count + 1
                    ELSE update_status.retry_count
                END
        """, (
            item_type,
            item_id,
            status,
            datetime.datetime.utcnow(),
            allow_update,
            allow_update,
            allow_update,
        ))
        conn.commit()


async def get_pending_items(item_type: str):
    async with get_async_connection() as conn:
        cur = conn.execute("SELECT item_id FROM update_status WHERE item_type=? AND status!='success'", (item_type,))
        return [row["item_id"] for row in cur.fetchall()]


# ----------------------------
# METADATA OPERATIONS
# ----------------------------

async def set_metadata(key: str, value: str):
    """Insert or update a metadata key/value."""
    async with get_async_connection() as conn:
        await conn.execute(
            """
            INSERT INTO metadata (key, value, updated_at)
            VALUES (?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = CURRENT_TIMESTAMP
            """,
            (key, value),
        )
        await conn.commit()


async def get_metadata(key: str) -> Optional[str]:
    """Fetch metadata value by key, or None if missing."""
    async with get_async_connection() as conn:
        cur = await conn.execute("SELECT value FROM metadata WHERE key = ?", (key,))
        row = await cur.fetchone()
        return row["value"] if row else None

# ----------------------------
# TEAM OPERATIONS
# ----------------------------

async def add_team(team_id: str, name: str, color: Optional[str], alt_id: Optional[str] = None):
    """Insert or update a team."""
    async with get_async_connection() as conn:
        conn.execute(
            "INSERT OR REPLACE INTO teams (id, alt_id, name, color) VALUES (?, ?, ?, ?)",
            (team_id, alt_id, name, color),
        )
        conn.commit()


async def get_team_by_id(team_id: str):
    async with get_async_connection() as conn:
        cur = conn.execute("SELECT * FROM teams WHERE id = ?", (team_id,))
        return cur.fetchone()


async def get_all_teams():
    async with get_async_connection() as conn:
        return conn.execute("SELECT * FROM teams").fetchall()

async def set_team_for_guild(guild_id: str, team_id: str):
    """Assign a guild to a team. Overrides previous assignment if it exists."""
    async with get_async_connection() as conn:
        conn.execute("""
            INSERT INTO guild_team (guild_id, team_id)
            VALUES (?, ?)
            ON CONFLICT(guild_id) DO UPDATE SET team_id=excluded.team_id
        """, (guild_id, team_id))
        conn.commit()


async def get_team_for_guild(guild_id: str):
    """Return the team a guild belongs to."""
    async with get_async_connection() as conn:
        cur = conn.execute("""
            SELECT t.* FROM teams t
            JOIN guild_team gt ON t.id = gt.team_id
            WHERE gt.guild_id = ?
        """, (guild_id,))
        return cur.fetchone()


async def get_guilds_in_team(team_id: str):
    """Return all guilds in a given team."""
    async with get_async_connection() as conn:
        cur = conn.execute("""
            SELECT g.* FROM guilds g
            JOIN guild_team gt ON g.id = gt.guild_id
            WHERE gt.team_id = ?
        """, (team_id,))
        return cur.fetchall()


# ----------------------------
# GUILD OPERATIONS
# ----------------------------


async def get_guild_by_id(guild_id: str):
    async with get_async_connection() as conn:
        cur = conn.execute("SELECT * FROM guilds WHERE id = ?", (guild_id,))
        return cur.fetchone()


async def get_guild_by_name(name: str):
    async with get_async_connection() as conn:
        cur = conn.execute("SELECT * FROM guilds WHERE name = ?", (name,))
        return cur.fetchone()

# ----------------------------
# MATCHUP OPERATIONS
# ----------------------------

async def add_matchup(team1_id: str, team2_id: str, team3_id: str):
    """Insert a new matchup (3 teams)."""
    async with get_async_connection() as conn:
        conn.execute(
            "INSERT INTO matchups (team1_id, team2_id, team3_id) VALUES (?, ?, ?)",
            (team1_id, team2_id, team3_id),
        )
        conn.commit()


async def get_matchups():
    """Return all matchups."""
    async with get_async_connection() as conn:
        return conn.execute("""
            SELECT m.id,
                   t1.name as team1,
                   t2.name as team2,
                   t3.name as team3
            FROM matchups m
            JOIN teams t1 ON m.team1_id = t1.id
            JOIN teams t2 ON m.team2_id = t2.id
            JOIN teams t3 ON m.team3_id = t3.id
        """).fetchall()


# ----------------------------
# QUICK DEMO
# ----------------------------

""" if __name__ == "__main__":
    init_db()

    # Example usage with hex-like IDs
    add_team("a1b2c3", "Blue Dragons", "alt-001", "blue")
    add_guild("deadbeef", "Guild of Light", "GLT", "a1b2c3")
    add_guild("cafebabe", "Shadow Guild", "SHD", "a1b2c3")

    print("Team:", dict(get_team_by_id("a1b2c3")))
    print("Guild by ID:", dict(get_guild_by_id("deadbeef")))
    print("Guild by Name:", dict(get_guild_by_name("Shadow Guild")))
    print("Team for Guild:", dict(get_team_for_guild("deadbeef")))
    print("Guilds in Team:", [dict(row) for row in get_guilds_in_team("a1b2c3")])

    # Add a matchup
    add_team("ff0011", "Red Phoenix", "alt-002", "red")
    add_team("00ff22", "Green Titans", "alt-003", "green")
    add_matchup("a1b2c3", "ff0011", "00ff22")

    print("Matchups:", [dict(row) for row in get_matchups()])
 """