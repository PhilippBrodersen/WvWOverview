import json
from datetime import datetime
import os

# Get current time
current_time = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

# Data to store
data = {
    "time": current_time
}

# Save to a JSON file that the website can read
p = os.path.join(os.path.dirname(__file__), 'test.txt')
with open(p, "w") as f:
    json.dump(data, f)