#!/usr/bin/env python3
import subprocess
import sys
import time
import os

# === CONFIG ===
GIT_REPO = "https://github.com/username/repo.git"  # your repo URL
APP_DIR = "."                                       # relative path to repo
MAIN_FILE = "main.py"                               # entry point
CHECK_INTERVAL = 60                                 # seconds between update checks

# === HELPERS ===
def update_repo():
    """Pull latest changes if any."""
    print("Checking for updates...")
    subprocess.run(["git", "fetch"], check=True)
    local = subprocess.check_output(["git", "rev-parse", "HEAD"]).strip()
    remote = subprocess.check_output(["git", "rev-parse", "@{u}"]).strip()
    if local != remote:
        print("New update found, pulling...")
        subprocess.run(["git", "pull"], check=True)
        return True
    return False

def run_app():
    """Start main.py"""
    return subprocess.Popen([sys.executable, MAIN_FILE], cwd=APP_DIR)

# === MAIN LOOP ===
if __name__ == "__main__":
    app_process = None
    while True:
        try:
            updated = update_repo()
            if app_process is None or updated:
                if app_process:
                    print("Restarting app...")
                    app_process.terminate()
                    app_process.wait()
                print("Starting app...")
                app_process = run_app()
        except Exception as e:
            print("Error:", e)

        # Wait before next check
        time.sleep(CHECK_INTERVAL)

        # Restart if crashed
        if app_process and app_process.poll() is not None:
            print("App crashed, restarting...")
            app_process = run_app()
