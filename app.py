import asyncio
import httpx
from fastapi import FastAPI
from database import one_time_update,get_matchup_hierarchy,get_team_for_guild_name,get_all_guilds_for_team, init_db, add_guild, get_guild_info, get_team_for_guild, get_all_matchups, get_team_name, get_guilds_for_team
from typing import Dict, Any
from tasks import update_teams, scheduler
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse
import os
from asyncio import get_running_loop
import tasks
import json
from fastapi.responses import JSONResponse
from pathlib import Path
from fastapi.responses import StreamingResponse

IMPORTANT_GUILD_PATH = Path(__file__).parent / "guilds.txt"

app = FastAPI()

app.mount("/static", StaticFiles(directory="static/frontend", html=True), name="frontend")



def read_important_guilds():
    file = open(IMPORTANT_GUILD_PATH, "r")
    lines = file.readlines()
    lines = [line.strip() for line in lines]
    return lines

@app.on_event("startup")
async def on_startup():
    """Initialize DB and perform first fetch."""
    print("init db")
    await init_db()
    print("db done")
    #await one_time_update()
    read_important_guilds()
    loop = get_running_loop()
    loop.create_task(scheduler())

@app.get("/")
async def serve_frontend():
    return FileResponse(os.path.join("static", "frontend", "index.html"))

@app.get("/importantguilds/")
async def get_important_guilds():
    return JSONResponse(content=read_important_guilds())

@app.get("/QoQ/")
async def get_alliance_team():
    team = await get_team_for_guild_name("Quality Ôver Quantity")
    return team or {"error": "Guild not found"}

@app.get("/data/")
async def get_data():
    if tasks.CACHE is None:
        return {}
    #return JSONResponse(content=tasks.CACHE)
    return tasks.CACHE

async def event_generator():
    last_cache = None
    last_guilds = None

    def make_message(cache, guilds):
        # Ensure cache is a dict
        if isinstance(cache, str):
            try:
                cache = json.loads(cache)
            except Exception:
                cache = {}
        elif not isinstance(cache, dict):
            cache = {}
        return {
            "data": cache,
            "important_guilds": guilds
        }

    # Initial message
    message = make_message(tasks.CACHE, read_important_guilds())
    last_cache = message["data"]
    last_guilds = message["important_guilds"]
    yield f"data: {json.dumps(message)}\n\n"

    while True:
        try:
            await asyncio.wait_for(tasks.cache_update_event.wait(), timeout=60.0)
            current_cache = tasks.CACHE
            current_guilds = read_important_guilds()

            message = make_message(current_cache, current_guilds)
            if message["data"] != last_cache or message["important_guilds"] != last_guilds:
                yield f"data: {json.dumps(message)}\n\n"
                last_cache = message["data"]
                last_guilds = message["important_guilds"]

            tasks.cache_update_event.clear()

        except asyncio.TimeoutError:
            yield ": keep-alive\n\n"

@app.get("/stream/data/")
async def stream_data():
    return StreamingResponse(event_generator(), media_type="text/event-stream")
