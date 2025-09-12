import asyncio
import httpx
from fastapi import FastAPI
from database import get_team_for_guild_name,get_all_guilds_for_team, init_db, add_guild, get_guild_info, get_team_for_guild, get_all_matchups, get_team_name, get_guilds_for_team
from typing import Dict, Any
from tasks import update_teams, scheduler
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse
import os
from asyncio import get_running_loop
from gw2api import semaphore



app = FastAPI()

app.mount("/static", StaticFiles(directory="static/frontend", html=True), name="frontend")

@app.on_event("startup")
async def on_startup():
    """Initialize DB and perform first fetch."""
    print("init db")
    await init_db()
    print("db done")
    global semaphore
    semaphore = asyncio.Semaphore(1)
    loop = get_running_loop()
    loop.create_task(scheduler())

@app.get("/")
async def serve_frontend():
    return FileResponse(os.path.join("static", "frontend", "index.html"))


@app.get("/guild/{guild_id}")
async def get_guild_data(guild_id: str):
    guild = await get_guild_info(guild_id)
    return guild

@app.get("/guild/team/{guild_id}")
async def get_guild_team(guild_id: str):
    team = await get_team_for_guild(guild_id)
    return team

@app.get("/team/name/{team_id}")
async def get_teamname(team_id: str):
    name = await get_team_name(team_id)
    return {"name": name}

@app.get("/team/guilds/{team_id}")
async def get_guilds_forteam(team_id: str):
    guilds = await get_all_guilds_for_team(team_id)
    return guilds

@app.get("/matches/")
async def get_matches():
    return await get_all_matchups()

@app.get("/QoQ/")
async def get_alliance_team():
    team = await get_team_for_guild_name("Quality Ôver Quantity")
    return team or {"error": "Guild not found"}