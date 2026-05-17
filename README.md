# JSOND (Json Daemon)

A fast, 90% spec-compliant Rust re-implementation of the v1 of npm package [`json-server`](https://github.com/typicode/json-server), built on **axum 0.8** + **tokio**.

---

## Installation

jjjfjf

## Table of Contents

- [JSOND (Json Daemon)](#jsond-json-daemon)
  - [Installation](#installation)
  - [Table of Contents](#table-of-contents)
  - [Usage](#usage)
    - [Examples](#examples)

## Usage

```console
jsond [OPTIONS] [DB]

Arguments:
  [DB]  Path to the JSON or JSON5 database file [default: db.json]

Options:
  -p, --port <PORT>                Port to listen on (0 = random) [env: PORT=] [default: 3000]
      --host <HOST>                Host address to bind to [env: HOST=] [default: 127.0.0.1]
  -s, --static <STATIC>            Serve static files from this directory [default: public]
      --delay <DELAY>              Add artificial delay in milliseconds to all responses [default: 0]
  -w, --watch                      Watch the database file for changes and reload automatically
      --cors                       Enable/Disable CORS headers [default: true]
      --readonly                   Readonly mode: disable POST, PUT, PATCH, DELETE
      --id-strategy <ID_STRATEGY>  [default: uuidv7] [possible values: int, uuidv4, uuidv7]
      --per-page <PER_PAGE>        Number of items per page [default: 10]
  -h, --help                       Print help
  -V, --version                    Print version
```

### Examples

```sh
# Basics
jsond db.json
jsond db.json5                                # JSON5 input supported
jsond db.json --port 4000 --host 0.0.0.0      # Use custom port and host
jsond db.json --id-strategy uuidv7            # Use uuidv4 for ids. default
jsond db.json --id-strategy uuidv4            # Use uuidv4 for ids
jsond db.json --id-strategy int               # Use incrementing integers for ids
jsond db.json --watch                         # Watch for file changes
jsond db.json --delay 500                     # Simulate 500ms network latency
jsond db.json --readonly                      # Readonly API (no writes)
jsond db.json --static ./public               # Serve static files. auto-detected if ./public exists
```
