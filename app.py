import asyncio
import httpx
from fastapi import FastAPI
from database import get_matchup_hierarchy,get_team_for_guild_name,get_all_guilds_for_team, init_db, add_guild, get_guild_info, get_team_for_guild, get_all_matchups, get_team_name, get_guilds_for_team
from typing import Dict, Any
from tasks import update_teams, scheduler
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse
import os
from asyncio import get_running_loop
import tasks
from fastapi.responses import JSONResponse

app = FastAPI()

app.mount("/static", StaticFiles(directory="static/frontend", html=True), name="frontend")

@app.on_event("startup")
async def on_startup():
    """Initialize DB and perform first fetch."""
    print("init db")
    await init_db()
    print("db done")
    loop = get_running_loop()
    loop.create_task(scheduler())

@app.get("/")
async def serve_frontend():
    return FileResponse(os.path.join("static", "frontend", "index.html"))

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