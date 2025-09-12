import uvicorn
from app import app

if __name__ == "__main__":
    uvicorn.run(
        "app:app",
        host="0.0.0.0",   # must be 0.0.0.0 for Uberspace
        port=12345,       # choose a free port between 1024-65535
        log_level="info",
        reload=False      # disable reload in production
    )
