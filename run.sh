#!/bin/bash

python_file="./json_updator.py"  # Replace with your Python file
python3.6 "$python_file"  # Bash will automatically wait until it finishes

python3.6 ./team_json_updator.py  # Bash will automatically wait until it finishes

destination_dir="/var/www/virtual/sleider/html"  # Replace with your destination

cp "./guilds.json" "$destination_dir"
cp "./teams.json" "$destination_dir"
cp "./matchups.json" "$destination_dir"

echo "Execution finished and files copied successfully!"