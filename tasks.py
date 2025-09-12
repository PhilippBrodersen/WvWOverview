import hashlib, json
from gw2api import fetch_all_wvw_guilds, fetch_guild_info, fetch_match
from database import set_matchup, get_metadata, set_metadata, set_new_guilds_pending, set_teams_for_guilds, add_guild, get_missing_guilds
from helper import checksum_json
import time
from datetime import datetime, timedelta
import asyncio

teams_lock = asyncio.Lock()  # prevents overlap
matchup_lock = asyncio.Lock()


async def scheduler():
    """Run update_teams every minute at hh:mm:05."""
    while True:
        print("hi")
        now = datetime.now()
        # Next full minute + 5s
        next_run = (now.replace(second=5, microsecond=0) 
                    + timedelta(minutes=1))
        wait_time = (next_run - now).total_seconds()
        await asyncio.sleep(wait_time)

        if not teams_lock.locked():
            asyncio.create_task(update_teams())
        else:
            print(f"Skipped update_teams at {datetime.now().time()} (still running)")

        if not matchup_lock.locked():
            asyncio.create_task(update_matchup())
        else:
            print(f"Skipped update_matchup at {datetime.now().time()} (still running)")

async def update_teams():
    """Checks if there are new WvW teams"""
    async with teams_lock:
        data = await fetch_all_wvw_guilds()

        print("updating")

        start = time.time()

        await set_teams_for_guilds(data)
        missing_guilds = await get_missing_guilds(data.keys())
        print(f"Found {len(missing_guilds)} new guilds")

        for guild_id in missing_guilds:
            try:
                guild = await fetch_guild_info(guild_id)
                await add_guild(guild_id, guild["name"], guild["tag"])
                print(f"Added {guild["name"]} [{guild["tag"]}]")
            except:
                 print(f"Guild {guild_id} can't be found")

        print("Execution took", time.time() - start, "seconds")

async def update_matchup():
     async with matchup_lock:
        print("Updating matchup...")
        start = time.time()
        for i in range(1, 6):
            try:
                match = await fetch_match(i)
                await set_matchup(i, match)
            except:
                print(f"Match update for tier {i} failed")
        print("update_matchup done, took", time.time() - start, "seconds")