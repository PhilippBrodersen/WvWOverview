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
from fastapi import FastAPI, Request, Response
from fastapi.responses import JSONResponse
import hashlib, json
from pathlib import Path

app = FastAPI()

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
async def get_data_endpoint(request: Request):
    payload = tasks.CACHE
    # Create ETag (can also just use last_updated timestamp instead of hash)
    etag_value = hashlib.md5(json.dumps(payload, sort_keys=True).encode()).hexdigest()

    # Check if client already has this version
    if request.headers.get("if-none-match") == etag_value:
        return Response(status_code=304)

    # Otherwise return fresh data with ETag
    return JSONResponse(
        content=payload,
        headers={"ETag": etag_value}
    )