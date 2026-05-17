# JSOND (Json Daemon)

A blazingly fast and 90% spec-compliant Rust reimplementation of the npm package [`json-server`](https://github.com/typicode/json-server).
Built on [Tokio](https://tokio.rs/) and [Axum](https://github.com/tokio-rs/axum).

---

## Quick Start

Create a `db.json` file:

```json
{
  "posts": [
    { "id": "1", "title": "Hello", "views": 100 },
    { "id": "2", "title": "World", "views": 200 }
  ],
  "comments": [{ "id": "1", "text": "Nice post", "postId": "1" }],
  "profile": {
    "name": "admin"
  }
}
```

Start the server:

```sh
jsond db.json
```

The server starts at `http://localhost:3000`.

Query your API:

```sh
curl http://localhost:3000/posts
curl http://localhost:3000/posts/1
curl http://localhost:3000/posts?views:gt=100
curl http://localhost:3000/posts?_sort=-views&_page=1&_per_page=10
```

## Table of Contents

- [JSOND (Json Daemon)](#jsond-json-daemon)
  - [Quick Start](#quick-start)
  - [Table of Contents](#table-of-contents)
  - [Installation](#installation)
    - [Option 1: Using Cargo](#option-1-using-cargo)
    - [Option 2: Using Install Script](#option-2-using-install-script)
    - [Option 3: Build from Source](#option-3-build-from-source)
  - [Usage](#usage)
    - [Database Format](#database-format)
      - [Collections (arrays)](#collections-arrays)
      - [Singletons (objects)](#singletons-objects)
    - [Basic Requests](#basic-requests)
    - [Filtering](#filtering)
    - [Nested Field Paths](#nested-field-paths)
    - [Multiple Filters (AND)](#multiple-filters-and)
    - [Complex Filters (`_where`)](#complex-filters-_where)
    - [Sorting](#sorting)
    - [Pagination](#pagination)
    - [Full-Text Search](#full-text-search)
    - [Relations](#relations)
    - [Cascading Deletes](#cascading-deletes)
  - [API Reference](#api-reference)
    - [Routes for Collections (Arrays)](#routes-for-collections-arrays)
    - [Routes for Singletons (Objects)](#routes-for-singletons-objects)
    - [Root Resource](#root-resource)
    - [Filter Operators](#filter-operators)
    - [Query Parameters](#query-parameters)
    - [Response Headers](#response-headers)
  - [CLI Reference](#cli-reference)
    - [Common Examples](#common-examples)
  - [Features](#features)
    - [Fully supported](#fully-supported)
    - [Partial/Not yet implemented](#partialnot-yet-implemented)
  - [Static Files](#static-files)
  - [ID Strategy](#id-strategy)
  - [Readonly Mode](#readonly-mode)
  - [File Watching](#file-watching)
  - [Testing and Development](#testing-and-development)
  - [License](#license)

## Installation

### Option 1: Using Cargo

Requires [Rust 1.84+](https://www.rust-lang.org/tools/install).

```sh
git clone git@github.com:princemuel/jsond.git
```

```sh
cargo install --path .
```

```sh
jsond db.json
```

### Option 2: Using Install Script

Download and run the install script:

```sh
curl -fsSL https://raw.githubusercontent.com/princemuel/jsond/main/install.sh | sh
jsond db.json
```

Or manually:

```sh
wget https://raw.githubusercontent.com/princemuel/jsond/main/install.sh
sh install.sh
jsond db.json
```

### Option 3: Build from Source

```sh
cargo build --release
./target/release/jsond db.json
```

---

## Usage

### Database Format

Create a `db.json` or `db.json5` file. Each top-level key becomes a REST resource.

```json
{
  "posts": [
    { "id": "1", "title": "a title", "views": 100 },
    { "id": "2", "title": "another title", "views": 200 }
  ],
  "comments": [
    { "id": "1", "text": "a comment about post 1", "postId": "1" },
    { "id": "2", "text": "another comment about post 1", "postId": "1" }
  ],
  "profile": {
    "name": "typicode"
  }
}
```

<details>

<summary>View db.json5 example</summary>

```json5
{
  posts: [
    { id: "1", title: "a title", views: 100 },
    { id: "2", title: "another title", views: 200 },
  ],
  comments: [
    { id: "1", text: "a comment about post 1", postId: "1" },
    { id: "2", text: "another comment about post 1", postId: "1" },
  ],
  profile: {
    name: "typicode",
  },
}
```

You can read more about the JSON5 format [here](https://github.com/json5/json5).

</details>

#### Collections (arrays)

```json
{
  "posts": [{ "id": "1", "title": "Hello", "author": "alice", "views": 100 }]
}
```

#### Singletons (objects)

```json
{
  "profile": {
    "name": "admin",
    "email": "admin@example.com"
  }
}
```

---

### Basic Requests

```sh
# List all posts
curl http://localhost:3000/posts

# Get a single post
curl http://localhost:3000/posts/1

# Create a post
curl -X POST http://localhost:3000/posts \
  -H "Content-Type: application/json" \
  -d '{"title":"New Post","views":0}'

# Update a post (full replace)
curl -X PUT http://localhost:3000/posts/1 \
  -H "Content-Type: application/json" \
  -d '{"title":"Updated","views":150}'

# Partially update a post
curl -X PATCH http://localhost:3000/posts/1 \
  -H "Content-Type: application/json" \
  -d '{"views":250}'

# Delete a post
curl -X DELETE http://localhost:3000/posts/1
```

### Filtering

Use `field=value` for equality or `field:operator=value` for conditions:

```sh
# Exact match
curl http://localhost:3000/posts?title=Hello

# Greater than
curl http://localhost:3000/posts?views:gt=100

# Less than or equal
curl http://localhost:3000/posts?views:lte=200

# Not equal
curl http://localhost:3000/posts?author:ne=alice

# In list
curl http://localhost:3000/posts?id:in=1,2,3

# Contains (case-insensitive substring)
curl http://localhost:3000/posts?title:contains=ello

# Starts with
curl http://localhost:3000/posts?title:startsWith=He

# Ends with
curl http://localhost:3000/posts?title:endsWith=o
```

### Nested Field Paths

Use dot notation to filter on nested properties:

```sh
curl http://localhost:3000/posts?author.name=alice
curl http://localhost:3000/posts?meta.tags:contains=rust
```

### Multiple Filters (AND)

Combine parameters with `&`:

```sh
# Posts with views > 50 AND title contains "rust"
curl 'http://localhost:3000/posts?views:gt=50&title:contains=rust'
```

### Complex Filters (`_where`)

For advanced queries, use `_where` with a JSON object:

```sh
# Posts with (views > 100) OR (author name < "m")
curl 'http://localhost:3000/posts?_where={"or":[{"views":{"gt":100}},{"author":{"name":{"lt":"m"}}}]}'

# Posts with (views >= 50) AND (title contains "rust")
curl 'http://localhost:3000/posts?_where={"and":[{"views":{"gte":50}},{"title":{"contains":"rust"}}]}'
```

### Sorting

Use `_sort` with comma-separated fields. Prefix with `-` for descending:

```sh
# Sort by title ascending
curl http://localhost:3000/posts?_sort=title

# Sort by views descending
curl http://localhost:3000/posts?_sort=-views

# Multi-field sort
curl http://localhost:3000/posts?_sort=author.name,-views
```

### Pagination

**Page-based** (returns envelope with metadata):

```sh
curl http://localhost:3000/posts?_page=1&_per_page=10
```

Response:

```json
{
  "first": 1,
  "prev": null,
  "next": 2,
  "last": 4,
  "pages": 4,
  "items": 100,
  "data": [...]
}
```

**Slice-based** (returns plain array with `X-Total-Count` header):

```sh
curl http://localhost:3000/posts?_start=0&_end=10
curl http://localhost:3000/posts?_start=0&_limit=5
```

Both methods include `X-Total-Count` header with the pre-pagination total.

### Full-Text Search

Search all string fields recursively (case-insensitive):

```sh
curl http://localhost:3000/posts?q=hello
```

### Relations

**Embed child records** (hasMany — uses `{parent}Id` convention):

```sh
# Attach comments to each post
curl http://localhost:3000/posts?_embed=comments
curl http://localhost:3000/posts/1?_embed=comments
```

Each post gets a `comments` array.

**Expand parent record** (belongsTo — uses `{parent}Id` field):

```sh
# Attach author to each post
curl http://localhost:3000/posts?_expand=author
curl http://localhost:3000/comments?_expand=post
```

Each child gets a parent object.

Both can be comma-separated for multiple relations:

```sh
curl http://localhost:3000/posts?_embed=comments,tags&_expand=author
```

### Cascading Deletes

Delete a resource and all dependents:

```sh
# Delete post 1 and all comments where postId == "1"
curl -X DELETE 'http://localhost:3000/posts/1?_dependent=comments'
```

---

## API Reference

### Routes for Collections (Arrays)

| Method | Route                         | Description                                        |
| ------ | ----------------------------- | -------------------------------------------------- |
| GET    | `/posts`                      | List all (supports filtering, sorting, pagination) |
| GET    | `/posts/:id`                  | Get single item                                    |
| POST   | `/posts`                      | Create item (id auto-generated if absent)          |
| PUT    | `/posts/:id`                  | Full replace                                       |
| PATCH  | `/posts/:id`                  | Partial update                                     |
| DELETE | `/posts/:id`                  | Delete item                                        |
| DELETE | `/posts/:id?_dependent=other` | Cascade delete                                     |

### Routes for Singletons (Objects)

| Method | Route      | Description           |
| ------ | ---------- | --------------------- |
| GET    | `/profile` | Get singleton         |
| PUT    | `/profile` | Replace singleton     |
| PATCH  | `/profile` | Merge-patch singleton |

### Root Resource

| Method | Route | Description             |
| ------ | ----- | ----------------------- |
| GET    | `/`   | List all resource names |

### Filter Operators

| Operator        | Example                | Description                   |
| --------------- | ---------------------- | ----------------------------- |
| (none) or `:eq` | `?title=hello`         | Exact equality                |
| `:ne`           | `?author:ne=bob`       | Not equal                     |
| `:lt`           | `?views:lt=50`         | Less than                     |
| `:lte`          | `?views:lte=100`       | Less than or equal            |
| `:gt`           | `?views:gt=100`        | Greater than                  |
| `:gte`          | `?views:gte=100`       | Greater than or equal         |
| `:in`           | `?id:in=1,2,3`         | Value in comma-separated list |
| `:contains`     | `?title:contains=rust` | Case-insensitive substring    |
| `:startsWith`   | `?title:startsWith=He` | Case-insensitive prefix       |
| `:endsWith`     | `?title:endsWith=lo`   | Case-insensitive suffix       |

### Query Parameters

| Parameter    | Example                | Description                         |
| ------------ | ---------------------- | ----------------------------------- |
| `_sort`      | `?_sort=title`         | Sort by field(s)                    |
| `-` (prefix) | `?_sort=-views`        | Descending order                    |
| `_page`      | `?_page=1`             | Page number (1-based)               |
| `_per_page`  | `?_per_page=10`        | Items per page (default: 10)        |
| `_start`     | `?_start=0`            | Start index (slice-based)           |
| `_end`       | `?_end=10`             | End index (slice-based)             |
| `_limit`     | `?_limit=5`            | Limit items (alternative to `_end`) |
| `q`          | `?q=hello`             | Full-text search                    |
| `_embed`     | `?_embed=comments`     | Include child records (hasMany)     |
| `_expand`    | `?_expand=author`      | Include parent record (belongsTo)   |
| `_where`     | `?_where={"or":[...]}` | Complex JSON filter                 |
| `_dependent` | `?_dependent=comments` | Cascade delete dependents           |

### Response Headers

| Header          | Description                                |
| --------------- | ------------------------------------------ |
| `X-Total-Count` | Total items before pagination              |
| `Content-Type`  | Always `application/json`                  |
| CORS headers    | Disabled by default (enable with `--cors`) |

---

## CLI Reference

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
      --cors                       Enable/Disable CORS headers [default: false]
      --readonly                   Readonly mode: disable POST, PUT, PATCH, DELETE
      --id-strategy <ID_STRATEGY>  [default: uuidv7] [possible values: int, uuidv4, uuidv7]
      --per-page <PER_PAGE>        Number of items per page [default: 10]
  -h, --help                       Print help
  -V, --version                    Print version
```

### Common Examples

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

## Features

### Fully supported

- Field filtering with operators (`:gt`, `:lt`, `:contains`, etc.)
- Nested dot-path filters
- Complex JSON filters with `_where`
- Multi-field sorting with direction control
- Page-based pagination with `_page` + `_per_page`
- Slice-based pagination with `_start` / `_end` / `_limit`
- `X-Total-Count` header on all list responses
- Full-text search with `q`
- Relations: `_embed` (hasMany) and `_expand` (belongsTo)
- Cascade delete with `_dependent`
- Singleton resources (GET/PUT/PATCH on objects)
- IDs always stored as strings
- Auto-generated IDs on POST (configurable strategy)
- Hot-reload on file changes
- CORS disabled by default
- Static file serving from `./public`
- JSON5 input format support

### Partial/Not yet implemented

- Custom middleware (through code integration)
- API Documentation
- TypeScript definitions
- GraphQL support

## Static Files

By default, jsond serves static files from the `./public` directory as a fallback.

```sh
mkdir public
echo '<h1>Hello</h1>' > public/index.html
jsond db.json
```

Access at `http://localhost:3000/`

Change the directory:

```sh
jsond db.json --static ./static
```

---

## ID Strategy

When creating resources via POST without an `id`, jsond auto-generates one:

| Strategy | Format         | Features                             | Default |
| -------- | -------------- | ------------------------------------ | ------- |
| `uuidv7` | `018e3d8c-…`   | Time-sortable, k-sortable in indexes | ✅      |
| `uuidv4` | `f47ac10b-…`   | Random, json-server v1 compatible    | ❌      |
| `int`    | `1, 2, 3, ...` | Incrementing integers, time-sortable | ❌      |

Use `--id-strategy` to change:

```sh
jsond db.json --id-strategy uuidv4
jsond db.json --id-strategy int
```

---

## Readonly Mode

Run with `--readonly` to disable all writes (POST, PUT, PATCH, DELETE):

```sh
jsond db.json --readonly
```

All write requests return `403 Forbidden`.

## File Watching

Run with `--watch` to automatically reload when the database file changes:

```sh
jsond db.json --watch
```

Useful for hand-editing JSON while the server runs.

## Testing and Development

Run integration tests:

```sh
cargo test
```

## License

MIT or Apache-2.0
