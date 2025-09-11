import asyncio
import httpx
from fastapi import FastAPI
from database import init_db, add_guild, get_guild_by_id
from typing import Dict, Any

# Semaphore: ensures only 1 external request at a time
semaphore = asyncio.Semaphore(1)

# Example guild ID
GUILD_ID = "A0BADA31-B57E-F011-81A9-8FB5CFBE7766"
semaphore = asyncio.Semaphore(1)
API_URL = f"https://api.guildwars2.com/v2/guild/{GUILD_ID}"

app = FastAPI()

async def fetch_json(url: str) -> dict:
    """Rate-limited GET returning JSON."""
    async with semaphore:
        async with httpx.AsyncClient() as client:
            resp = await client.get(url, timeout=10.0)
            resp.raise_for_status()
            data = resp.json()
            await asyncio.sleep(0.21)  # enforce ≥0.2s between requests
            return data


async def fetch_external_guild(guild_id: str) -> dict:
    """Fetch guild data from external API, rate-limited."""
    async with semaphore:
        async with httpx.AsyncClient() as client:
            resp = await client.get(f"https://api.guildwars2.com/v2/guild/{guild_id}", timeout=10.0)
            resp.raise_for_status()
            data = resp.json()
            await asyncio.sleep(0.21)  # ensure ≥0.2s between calls
            return data


async def initial_fetch():
    """Fetch guild info once on startup and save to DB + cache."""
    print("[Startup] Fetching guild data...")
    data = await fetch_external_guild(GUILD_ID)

    guild_id = data["id"]
    name = data["name"]
    tag = data["tag"]

    # Save to DB
    add_guild(guild_id, name, tag, team_id=None)

    print(f"[Startup] Guild {name} ({tag}) cached and stored.")


@app.on_event("startup")
async def on_startup():
    """Initialize DB and perform first fetch."""
    init_db()
    await initial_fetch()

@app.get("/")
def read_root():
    return {"Hello": "World"}

@app.get("/guild/{guild_id}")
def get_guild_data(guild_id: str):
    """Return guild data from memory cache (fast)."""
    guild = get_guild_by_id(guild_id)
    return guild
