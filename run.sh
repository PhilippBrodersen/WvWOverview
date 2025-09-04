#!/bin/bash

# --- Part 1: Execute a Python file and wait until it's finished ---
python_file="./json_updator.py"  # Replace with your Python file
python3.6 "$python_file"  # Bash will automatically wait until it finishes

# --- Part 2: Copy 3 files to a destination directory ---
files_to_copy=("./" "file2.txt" "file3.txt")  # Replace with your files
destination_dir="/var/www/virtual/sleider/html"  # Replace with your destination

cp "./guilds.json" "$destination_dir"
cp "./teams.json" "$destination_dir"

echo "Execution finished and files copied successfully!"