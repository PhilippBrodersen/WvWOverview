import requests
import time
import json

team_names = {
    "11001": "Moogooloo",
    "11002": "Rall's Rest",
    "11003": "Domain of Torment",
    "11004": "Yohlon Haven",
    "11005": "Tombs of Drascir",
    "11006": "Hall of Judgment",
    "11007": "Throne of Balthazar",
    "11008": "Dwayna's Temple",
    "11009": "Abaddon's Prison",
    "11010": "Cathedral of Blood",
    "11011": "Lutgardis Conservatory",
    "11012": "Mosswood",
    "12001": "Skrittsburgh",
    "12002": "Fortune's Vale",
    "12003": "Silent Woods",
    "12004": "Ettin's Back",
    "12005": "Domain of Anguish",
    "12006": "Palawadan",
    "12007": "Bloodstone Gulch",
    "12008": "Frost Citadel",
    "12009": "Dragrimmar",
    "12010": "Grenth's Door",
    "12011": "Mirror of Lyssa",
    "12012": "Melandru's Dome",
    "12013": "Kormir's Library",
    "12014": "Great House Aviary",
    "12015": "Bava Nisos",
}

MAX_ROUNDS = 5
RETRY_DELAY = 10  # seconds

# --- Fetch guild list ---
guild_list = requests.get("https://api.guildwars2.com/v2/wvw/guilds/eu").json()

teams_data = {}
guilds_data = {}
remaining_guilds = list(guild_list.items())

def fetch_guild(guild_id, team_id):
    """Fetch guild info, return dict or raise exception."""
    time.sleep(0.25)
    data = requests.get(f"https://api.guildwars2.com/v2/guild/{guild_id}", timeout=10).json()
    name = data.get("name", "???")
    tag = data.get("tag", "[???]")
    return {"name": name, "tag": tag, "team_id": team_id}

for round_num in range(1, MAX_ROUNDS + 1):
    print(f"\n--- Round {round_num}/{MAX_ROUNDS} ---")
    next_round = []

    for idx, (guild_id, team_id) in enumerate(remaining_guilds, start=1):
        if idx==20:
            break

        try:
            guild_info = fetch_guild(guild_id, team_id)
            guilds_data[guild_id] = guild_info

            teams_data.setdefault(team_id, {"team_id": team_id, "team_name": guild_info["team_name"], "guilds": []})
            if guild_id not in teams_data[team_id]["guilds"]:
                teams_data[team_id]["guilds"].append(guild_id)

            print(f"({idx}/{len(remaining_guilds)}) {guild_info['name']} [{guild_info['tag']}] in {guild_info['team_name']}")
        except Exception as e:
            print(f"Error fetching {guild_id}: {e}")
            next_round.append((guild_id, team_id))

    if not next_round:
        break  # all done

    remaining_guilds = next_round
    if round_num < MAX_ROUNDS:
        print(f"{len(remaining_guilds)} guilds failed, retrying after {RETRY_DELAY}s...")
        time.sleep(RETRY_DELAY)
    else:
        print(f"{len(remaining_guilds)} guilds failed after {MAX_ROUNDS} rounds, skipping.")

# --- Save JSON files ---
with open("/home/p/Desktop/teams.json", "w", encoding="utf-8") as f:
    json.dump(list(teams_data.values()), f, ensure_ascii=False, indent=2)

with open("/home/p/Desktop/guilds.json", "w", encoding="utf-8") as f:
    json.dump(guilds_data, f, ensure_ascii=False, indent=2)
