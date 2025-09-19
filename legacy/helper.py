import json
import hashlib

def checksum_json(data: dict) -> str:
    blob = json.dumps(data, sort_keys=True).encode("utf-8")
    return hashlib.sha256(blob).hexdigest()