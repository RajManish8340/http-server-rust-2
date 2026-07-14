# Rust HTTP Server

A simple HTTP server built from scratch in Rust, created as part of the [CodeCrafters HTTP Server challenge](https://codecrafters.io/challenges/http-server).

## What it does

This server listens on `127.0.0.1:4221` and handles HTTP/1.1 requests. It supports:

- Multiple **concurrent connections** via threads
- **Persistent connections** (HTTP/1.1 keep-alive by default)
- **Explicit connection closure** via `Connection: close` header
- **gzip compression** via `Accept-Encoding: gzip` header

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Returns `200 OK` |
| GET | `/echo/{str}` | Returns the `{str}` back in the response body |
| GET | `/user-agent` | Returns the client's `User-Agent` header value |
| GET | `/files/{filename}` | Returns the contents of the file from the configured directory |
| POST | `/files/{filename}` | Creates a new file in the configured directory with the request body |
| ANY | anything else | Returns `404 Not Found` |

---

## How to run

```bash
cargo run --directory /tmp/
```
or if you want default dir as root of the project
```bash
cargo run
```

The `--directory` flag tells the server where to read and write files.

---

## How to test each endpoint

### 1. Root endpoint
```bash
curl -v http://localhost:4221/
```
Expected response:
```
HTTP/1.1 200 OK
```

---

### 2. Echo endpoint
```bash
curl -v http://localhost:4221/echo/hello
```
Expected response:
```
HTTP/1.1 200 OK
Content-Type: text/plain
Content-Length: 5

hello
```

---

### 3. Echo with gzip compression
```bash
curl -v -H "Accept-Encoding: gzip" http://localhost:4221/echo/hello | hexdump -C
```
Expected response:
```
HTTP/1.1 200 OK
Content-Type: text/plain
Content-Encoding: gzip
Content-Length: 25
```
The body will be binary gzip data starting with `1f 8b` (gzip magic bytes).

If the client sends an unsupported encoding, the server responds without `Content-Encoding` and returns the plain body.

---

### 4. User-Agent endpoint
```bash
curl -v http://localhost:4221/user-agent
```
Expected response:
```
HTTP/1.1 200 OK
Content-Type: text/plain
Content-Length: 11

curl/8.5.0
```

---

### 5. GET a file
First create a file in your directory:
```bash
echo "hello world" > /tmp/test.txt
```

Then request it:
```bash
curl -v http://localhost:4221/files/test.txt
```
Expected response:
```
HTTP/1.1 200 OK
Content-Type: application/octet-stream
Content-Length: 12

hello world
```

If the file does not exist:
```
HTTP/1.1 404 Not Found
```

---

### 6. POST a file
```bash
curl -v --data "12345" -H "Content-Type: application/octet-stream" http://localhost:4221/files/file_123
```
Expected response:
```
HTTP/1.1 201 Created
```

This creates a file at `/tmp/file_123` containing `12345`. Verify it:
```bash
cat /tmp/file_123
# 12345
```

---

### 7. Persistent connections
Send multiple requests over the same connection:
```bash
curl --http1.1 -v http://localhost:4221/echo/banana --next http://localhost:4221/user-agent -H "User-Agent: test"
```
Both requests reuse the same TCP connection (`Re-using existing connection` in curl output).

---

### 8. Explicit connection closure
```bash
curl --http1.1 -v http://localhost:4221/echo/orange --next http://localhost:4221/ -H "Connection: close"
```
- First request: connection stays open
- Second request: server echoes `Connection: close` in response and closes the connection

---

### 9. Concurrent connections
```bash
curl --http1.1 -v http://localhost:4221/echo/apple --next http://localhost:4221/echo/mango &
curl --http1.1 -v http://localhost:4221/echo/banana --next http://localhost:4221/user-agent -H "User-Agent: test" &
```
Both connections are handled simultaneously, each keeping their own persistent state.

---

## How it works internally

### Request parsing
Every HTTP request looks like this:
```
GET /echo/abc HTTP/1.1\r\n
Host: localhost:4221\r\n
User-Agent: curl/8.5.0\r\n
\r\n
```

The server reads it in three steps:
1. **Request line** — first line, gives method and path
2. **Headers** — subsequent lines until an empty line
3. **Body** — remaining bytes (only for POST, read using `Content-Length`)

### Concurrency
Each incoming connection is handled in its own thread using `std::thread::spawn`, so multiple clients can connect at the same time without blocking each other.

### Persistent connections
Each thread runs a `loop` — after responding to a request, it loops back and waits for the next one on the same TCP connection. The loop breaks when:
- The client sends `Connection: close`
- The client closes the connection (empty request line)

### gzip compression
If the request includes `Accept-Encoding: gzip` (or a comma-separated list containing `gzip`), the server compresses the response body using the `flate2` crate and adds `Content-Encoding: gzip` to the response. The `Content-Length` reflects the **compressed** size.

### File serving
The `--directory` flag sets the base directory at startup. All file reads and writes use this directory as the root.
