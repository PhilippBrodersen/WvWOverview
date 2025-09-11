import asyncio
import httpx
from database import init_db, add_guild, get_guild_by_id
from typing import Dict, Any


# Semaphore: ensures only 1 external request at a time
semaphore = asyncio.Semaphore(1)

# Example guild ID
GUILD_ID = "A0BADA31-B57E-F011-81A9-8FB5CFBE7766"
API_URL = f"https://api.guildwars2.com/v2/guild/{GUILD_ID}"

semaphore = asyncio.Semaphore(1)
async def fetch_json(url: str) -> dict:
    async with semaphore:
        async with httpx.AsyncClient() as client:
            resp = await client.get(url, timeout=10.0)
            resp.raise_for_status()
            data = resp.json()
            await asyncio.sleep(0.21)  # enforce ≥0.2s between requests
            return data

async def fetch_all_wvw_guilds() -> list:
    data = await fetch_json("https://api.guildwars2.com/v2/wvw/guilds/eu")
    guilds: list[Dict] = []

    for guild_id in data.keys():  
        guild = await fetch_guild_info(guild_id)
        guilds.append(guild)

    return guilds



async def fetch_guild_info(guild_id: str) -> dict:
    return await fetch_json(f"https://api.guildwars2.com/v2/guild/{GUILD_ID}")