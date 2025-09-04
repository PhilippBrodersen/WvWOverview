#!/bin/bash

git pull

destination_dir="/var/www/virtual/sleider/html"  # Replace with your destination

cp "./index.html" "$destination_dir"

echo "Update done"