import sqlite3
import datetime
from pathlib import Path
from typing import Optional

DB_PATH = Path(__file__).parent / "app.db"


def get_connection():
    """Open a database connection with WAL mode enabled."""
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row  # return dict-like rows
    conn.execute("PRAGMA journal_mode=WAL;")
    return conn


def init_db():
    """Create tables if they don't exist."""
    with get_connection() as conn:
        cur = conn.cursor()

        # Teams
        cur.execute("""
        CREATE TABLE IF NOT EXISTS teams (
            id TEXT PRIMARY KEY,       -- teamid is a hex string
            alt_id TEXT UNIQUE,
            name TEXT NOT NULL,
            color TEXT
        );
        """)

        # Guilds
        cur.execute("""
        CREATE TABLE IF NOT EXISTS guilds (
            id TEXT PRIMARY KEY,       -- guildid is a hex string
            name TEXT NOT NULL,
            tag TEXT
        );
        """)

        cur.execute("""
            CREATE TABLE IF NOT EXISTS guild_team (
            guild_id TEXT PRIMARY KEY,     -- each guild belongs to only one team
            team_id TEXT NOT NULL,
            FOREIGN KEY (guild_id) REFERENCES guilds(id) ON DELETE CASCADE,
            FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE CASCADE
        );
        """)

        # Matchups: each entry has 3 teams
        cur.execute("""
        CREATE TABLE IF NOT EXISTS matchups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            team1_id TEXT NOT NULL,
            team2_id TEXT NOT NULL,
            team3_id TEXT NOT NULL,
            FOREIGN KEY (team1_id) REFERENCES teams(id),
            FOREIGN KEY (team2_id) REFERENCES teams(id),
            FOREIGN KEY (team3_id) REFERENCES teams(id)
        );
        """)

        cur.execute("""
        CREATE TABLE IF NOT EXISTS update_status (
            item_type TEXT NOT NULL,      -- 'guild', 'team', etc
            item_id TEXT NOT NULL,        -- guild_id, team_id, etc
            status TEXT NOT NULL DEFAULT 'pending',   -- 'pending', 'success', 'failed'
            last_attempt TIMESTAMP,
            retry_count INTEGER DEFAULT 0,
            PRIMARY KEY (item_type, item_id)
        );
        """)

        conn.commit()


# ----------------------------
# STATUS OPERATIONS
# ----------------------------

def mark_status(item_type: str, item_id: str, status: str):
    with get_connection() as conn:
        conn.execute("""
            INSERT INTO update_status (item_type, item_id, status, last_attempt, retry_count)
            VALUES (?, ?, ?, ?, 0)
            ON CONFLICT(item_type, item_id) DO UPDATE SET
                status=excluded.status,
                last_attempt=excluded.last_attempt,
                retry_count=CASE WHEN excluded.status='failed' THEN retry_count+1 ELSE retry_count END
        """, (item_type, item_id, status, datetime.datetime.utcnow()))
        conn.commit()


def get_pending_items(item_type: str):
    with get_connection() as conn:
        cur = conn.execute("SELECT item_id FROM update_status WHERE item_type=? AND status!='success'", (item_type,))
        return [row["item_id"] for row in cur.fetchall()]


# ----------------------------
# TEAM OPERATIONS
# ----------------------------

def add_team(team_id: str, name: str, color: Optional[str], alt_id: Optional[str] = None):
    """Insert or update a team."""
    with get_connection() as conn:
        conn.execute(
            "INSERT OR REPLACE INTO teams (id, alt_id, name, color) VALUES (?, ?, ?, ?)",
            (team_id, alt_id, name, color),
        )
        conn.commit()


def get_team_by_id(team_id: str):
    with get_connection() as conn:
        cur = conn.execute("SELECT * FROM teams WHERE id = ?", (team_id,))
        return cur.fetchone()


def get_all_teams():
    with get_connection() as conn:
        return conn.execute("SELECT * FROM teams").fetchall()

def set_team_for_guild(guild_id: str, team_id: str):
    """Assign a guild to a team. Overrides previous assignment if it exists."""
    with get_connection() as conn:
        conn.execute("""
            INSERT INTO guild_team (guild_id, team_id)
            VALUES (?, ?)
            ON CONFLICT(guild_id) DO UPDATE SET team_id=excluded.team_id
        """, (guild_id, team_id))
        conn.commit()


def get_team_for_guild(guild_id: str):
    """Return the team a guild belongs to."""
    with get_connection() as conn:
        cur = conn.execute("""
            SELECT t.* FROM teams t
            JOIN guild_team gt ON t.id = gt.team_id
            WHERE gt.guild_id = ?
        """, (guild_id,))
        return cur.fetchone()


def get_guilds_in_team(team_id: str):
    """Return all guilds in a given team."""
    with get_connection() as conn:
        cur = conn.execute("""
            SELECT g.* FROM guilds g
            JOIN guild_team gt ON g.id = gt.guild_id
            WHERE gt.team_id = ?
        """, (team_id,))
        return cur.fetchall()


# ----------------------------
# GUILD OPERATIONS
# ----------------------------

def add_guild(guild_id: str, name: str, tag: str, team_id: Optional[str]):
    """Insert or update a guild."""
    with get_connection() as conn:
        conn.execute(
            "INSERT OR REPLACE INTO guilds (id, name, tag, team_id) VALUES (?, ?, ?, ?)",
            (guild_id, name, tag, team_id),
        )
        conn.commit()

async def fetch_guild_info(guild_id: str) -> dict:
    """Fetch a single guild and update DB + cache."""
    try:
        data = await fetch_json(f"https://api.guildwars2.com/v2/guild/{guild_id}")
        name = data.get("name", "Unknown")
        tag = data.get("tag", "UNK")
        add_guild(guild_id, name, tag, team_id=None)
        mark_status("guild", guild_id, "success")
        return data
    except Exception as e:
        mark_status("guild", guild_id, "failed")
        return {}


def get_guild_by_id(guild_id: str):
    with get_connection() as conn:
        cur = conn.execute("SELECT * FROM guilds WHERE id = ?", (guild_id,))
        return cur.fetchone()


def get_guild_by_name(name: str):
    with get_connection() as conn:
        cur = conn.execute("SELECT * FROM guilds WHERE name = ?", (name,))
        return cur.fetchone()


def get_team_for_guild(guild_id: str):
    """Return the team a guild belongs to."""
    with get_connection() as conn:
        cur = conn.execute("""
            SELECT t.* FROM teams t
            JOIN guilds g ON g.team_id = t.id
            WHERE g.id = ?
        """, (guild_id,))
        return cur.fetchone()


def get_guilds_in_team(team_id: str):
    """Return all guilds in a given team."""
    with get_connection() as conn:
        return conn.execute("SELECT * FROM guilds WHERE team_id = ?", (team_id,)).fetchall()


# ----------------------------
# MATCHUP OPERATIONS
# ----------------------------

def add_matchup(team1_id: str, team2_id: str, team3_id: str):
    """Insert a new matchup (3 teams)."""
    with get_connection() as conn:
        conn.execute(
            "INSERT INTO matchups (team1_id, team2_id, team3_id) VALUES (?, ?, ?)",
            (team1_id, team2_id, team3_id),
        )
        conn.commit()


def get_matchups():
    """Return all matchups."""
    with get_connection() as conn:
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

if __name__ == "__main__":
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
