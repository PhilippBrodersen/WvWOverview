import asyncio
import httpx
from fastapi import FastAPI
from database import init_db, add_guild, get_guild_by_id
from typing import Dict, Any
from tasks import update_teams, scheduler


app = FastAPI()

@app.on_event("startup")
async def on_startup():
    """Initialize DB and perform first fetch."""
    print("init db")
    await init_db()
    print("db done")
    asyncio.create_task(scheduler())

@app.get("/")
def read_root():
    return {"Hello": "World"}

@app.get("/guild/{guild_id}")
def get_guild_data(guild_id: str):
    """Return guild data from memory cache (fast)."""
    guild = get_guild_by_id(guild_id)
    return guild
