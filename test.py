import requests

endpoints = [
    ("Road Light", "http://127.0.0.1:8080/road/3/2/1.png"),
    ("Road Dark", "http://127.0.0.1:8080/dark/3/2/1.png"),
    ("Satellite", "http://127.0.0.1:8080/satellite/3/2/1.jpg"),
    ("Direct Root", "http://127.0.0.1:8080/3/2/1.jpg"),
    ("Overlay", "http://127.0.0.1:8080/overlay/3/2/1.png"),
    ("Hybrid", "http://127.0.0.1:8080/hybrid/3/2/1.jpg"),
    ("Road Custom", "http://127.0.0.1:8080/road/3/2/1.png?tint=dark&scale=2&poi=0&labels=0"),
    ("Dark Retina", "http://127.0.0.1:8080/dark/3/2/1.png?scale=2&emphasis=muted"),
    ("Icon", "http://127.0.0.1:8080/md/v1/icon?name=airport&style=0"),
    ("Shield", "http://127.0.0.1:8080/md/v1/shield?name=i-80&style=0"),
]

for name, url in endpoints:
    try:
        r = requests.get(url, timeout=10)
        print(f"{name:15} -> Status: {r.status_code}, Length: {len(r.content):6}, Type: {r.headers.get('Content-Type')}, CORS: {r.headers.get('Access-Control-Allow-Origin')}")
    except Exception as e:
        print(f"{name:15} -> Error: {e}")
