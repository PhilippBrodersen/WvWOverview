import hashlib, json
from gw2api import fetch_all_wvw_guilds, fetch_guild_info, fetch_match
from database import get_matchup_hierarchy, set_matchup,  set_new_guilds_pending, set_teams_for_guilds, add_guild, get_missing_guilds
from helper import checksum_json
import time
from datetime import datetime, timedelta
import asyncio

teams_lock = asyncio.Lock()  # prevents overlap
matchup_lock = asyncio.Lock()


async def scheduler():
    await update_dashboard_cache()
    while True:
        now = datetime.now()
        next_run = (now.replace(second=5, microsecond=0) + timedelta(minutes=1))
        wait_time = (next_run - now).total_seconds()
        await asyncio.sleep(wait_time)

        loop = asyncio.get_running_loop()

        tasks = []

        if not teams_lock.locked():
            tasks.append(loop.create_task(update_teams()))
        else:
            print(f"Skipped update_teams at {datetime.now().time()} (still running)")

        if not matchup_lock.locked():
            tasks.append(loop.create_task(update_matchup()))
        else:
            print(f"Skipped update_matchup at {datetime.now().time()} (still running)")

        if tasks:
            # Wait for all updates to finish before refreshing CACHE
            await asyncio.gather(*tasks)
            await update_dashboard_cache()

CACHE = None
cache_update_event = asyncio.Event()
async def update_dashboard_cache():
    global CACHE
    hierarchy = await get_matchup_hierarchy()
    #CACHE = json.dumps(hierarchy)
    CACHE = hierarchy
    cache_update_event.set()  # signal that CACHE changed
    cache_update_event.clear()  # reset for next change
    print("CACHE updated")


async def fetch_and_add_guild(guild_id: str):
    """Fetch guild info and add it to DB."""
    try:
        guild = await fetch_guild_info(guild_id)
        await add_guild(guild_id, guild["name"], guild["tag"])
        print(f"Added {guild['name']} [{guild['tag']}]")
    except Exception:
        print(f"Guild {guild_id} can't be found")

async def update_teams():
    """Checks if there are new WvW teams"""
    async with teams_lock:
        data = await fetch_all_wvw_guilds()

        print("updating")

        start = time.time()

        await set_teams_for_guilds(data)
        missing_guilds = await get_missing_guilds(data.keys())
        print(f"Found {len(missing_guilds)} new guilds")

        await asyncio.gather(*(fetch_and_add_guild(gid) for gid in missing_guilds))

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