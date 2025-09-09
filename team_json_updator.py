import requests
import json
import os

# List of match URLs
urls = [
    "https://api.guildwars2.com/v2/wvw/matches/2-1",
    "https://api.guildwars2.com/v2/wvw/matches/2-2",
    "https://api.guildwars2.com/v2/wvw/matches/2-3",
    "https://api.guildwars2.com/v2/wvw/matches/2-4",
    "https://api.guildwars2.com/v2/wvw/matches/2-5"
]

results = []

for url in urls:
    response = requests.get(url)
    if response.status_code == 200:
        data = response.json()
        worlds = data.get("worlds", {})
        modified_worlds = {k: int(f"2{v}") for k, v in worlds.items()}
        results.append(modified_worlds)
    else:
        print(f"Failed to fetch {url}, status code {response.status_code}")
        results.append({})


script_dir = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(script_dir, "matchup.json"), "w", encoding="utf-8") as f:
    json.dump(results, f, ensure_ascii=False, indent=4)