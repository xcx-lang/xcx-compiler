# XCX 3.0 JSON and HTTP

## JSON

JSON objects in XCX 3.0 are **mutable**.

### Creation

```xcx
json: config <<< {"port": 8080, "debug": false} >>>;
json: user   <<< {"name": "", "age": 0} >>>;
```

Values in the literal can be placeholders (`""`, `0`, `false`) to be filled later via `.set()`.

### Serialization from Collections
You can also create JSON objects and arrays directly from XCX collections using the `.toJson()` method. This is available for:
- **Maps**: Returns a JSON object.
- **Tables**: Returns a JSON array of objects.

See the [Collections Documentation](collections.md) for more details on mapping and behavior.

### Parsing

```xcx
json: parsed = json.parse(raw_string);
```

> [!CAUTION]
> **Panic on Invalid JSON (R305)**: If parsing fails, the VM terminates immediately. Verify string content before parsing.

### Mutability Pattern

Declare a schema with zero-values, then populate:

```xcx
json: resp <<< {"token": "", "role": "", "uid": 0, "ok": false} >>>;
resp.set("token", crypto.token(32));
resp.set("role",  "admin");
resp.set("uid",   42);
resp.set("ok",    true);
yield net.respond(200, resp);
```

### JSON Methods

| Method                    | Signature             | Returns | Description                                                    |
|---------------------------|-----------------------|---------|----------------------------------------------------------------|
| `.exists(path)`           | `(s) → b`             | `b`     | Checks if path exists and is non-null                          |
| `.get(path/idx)`          | `(s/i) → json`        | `json`  | Gets element at path or index                                  |
| `.bind(path, var)`        | `(s, ref) → b`        | `b`     | Extracts value into a **pre-declared** XCX variable            |
| `.set(path, val)`         | `(s, T) → b`          | `b`     | Sets value at path; creates key if missing                     |
| `.push(val)`              | `(json) → b`          | `b`     | Appends element to a JSON array node                           |
| `.size()` / `.count()`    | `() → i`              | `i`     | Number of keys (object) or elements (array)                    |
| `.toStr()`                | `() → s`              | `s`     | Serializes to JSON string                                      |
| `.inject(path, map, tbl)` | `(s, map, table) → b` | `b`     | Bulk import of JSON array into XCX table                       |
| `.first()`                | `() → json`           | `json`  | Returns the first element of a JSON array; halt.error if empty |

> [!NOTE]
> **`.push()` on JSON arrays**: `.push()` works exclusively on JSON nodes that are arrays (`[]`). Calling `.push()` on a JSON object (`{}`) results in a `halt.error`.
>
> ```xcx
> json: data <<< {"items": []} >>>;
> json: obj <<< {"id": 1} >>>;
> data.get("items").push(obj);   --- OK: "items" is an array
> data.push(obj);                --- halt.error: data is an object, not an array
> ```

> [!IMPORTANT]
> **`.bind()` syntax**: The second argument must be a **previously declared variable**. You cannot declare the type inline.
>
> ```xcx
> --- Wrong
> req.bind("ip", s: ip);
>
> --- Correct
> s: ip;
> req.bind("ip", ip);
> ```

### Path Notation

Both dot-notation and bracket notation are supported for nested access:

```xcx
--- Nested field
json: cfg <<< {"server": {"host": "localhost"}} >>>;
s: host;
cfg.bind("server.host", host);

--- Array index
json: resp <<< {"items": []} >>>;
resp.set("items[0]", first_item);
resp.set("items[1]", second_item);
```

### .inject() — Bulk Import

Import a JSON array directly into an XCX table:

```xcx
json: data <<< {"users":[{"id":1,"name":"Alice"},{"id":2,"name":"Bob"}]} >>>;
table: imported { columns=[uid::i, uname::s] rows=[EMPTY] };
map: mapping { schema=[s<->s] data=["uid"::"id", "uname"::"name"] };
data.inject("users", mapping, imported);
imported.show();
```

### JSON in HTTP Requests

Inside HTTP handlers, `json: req` has this structure:

```json
{
    "method":  "POST",
    "path":    "/api/login",
    "query":   { "page": "1" },
    "headers": { "authorization": "Bearer ..." },
    "body":    { ... },
    "ip":      "1.2.3.4"
}
```

```xcx
s: ip;
req.bind("ip", ip);

json: body;
req.bind("body", body);

json: headers;
req.bind("headers", headers);

s: auth;
headers.bind("authorization", auth);
```

---

## HTTP Client

### High-Level API

| Method                 | Signature          | Returns |
|------------------------|--------------------|---------|
| `net.get(url)`         | `(s) → json`       | `json`  |
| `net.post(url, body)`  | `(s, json) → json` | `json`  |
| `net.put(url, body)`   | `(s, json) → json` | `json`  |
| `net.delete(url)`      | `(s) → json`       | `json`  |

```xcx
json: resp = net.get("https://api.example.com/users");

json: body <<< {"name": "Alice"} >>>;
json: resp = net.post("https://api.example.com/users", body);
```

### Response Object

```json
{
    "status":  200,
    "ok":      true,
    "body":    { ... },
    "headers": { "content-type": "application/json" },
    "text":    "...",
    "error":   "..."
}
```

| Field     | Type   | Description                                                       |
|-----------|--------|-------------------------------------------------------------------|
| `status`  | `i`    | HTTP status code                                                  |
| `ok`      | `b`    | `true` when status >= 200 and < 300                               |
| `body`    | `json` | Parsed response body (if JSON); otherwise `null`                  |
| `headers` | `json` | Response headers                                                  |
| `text`    | `s`    | Response body as raw string (always available)                    |
| `error`   | `s`    | Error message if request failed; empty string if OK               |

`ok` is `true` when status >= 200 and < 300. Always check `ok` before accessing `body`.

```xcx
json: resp = net.get("https://api.example.com/data");
if (resp.ok) then;
    --- working with resp.body
else;
    s: err;
    if (resp.exists("error")) then;
        resp.bind("error", err);
    end;
    >! "Error: " + s(resp.get("status")) + " | " + err;
end;
```

### Low-Level Builder (`net.request`)

```xcx
net.request {
    method  = "POST",
    url     = "https://api.example.com/data",
    headers = ["Authorization" :: "Bearer xyz", "X-App" :: "XCX"],
    body    = my_json,
    timeout = 5000
} as resp;
```

| Field     | Type        | Required | Default  | Description                           |
|-----------|-------------|----------|----------|---------------------------------------|
| `method`  | `s`         | Yes      | —        | `"GET"`, `"POST"`, `"PUT"`, etc.      |
| `url`     | `s`         | Yes      | —        | Full URL with scheme                  |
| `headers` | `map:s<->s` | No       | `{}`     | Additional request headers            |
| `body`    | `json`      | No       | `null`   | Ignored for GET and DELETE            |
| `timeout` | `i`         | No       | `10000`  | Milliseconds                          |

---

## HTTP Server

### serve Directive

```xcx
serve: app {
    port    = 8080,
    host    = "0.0.0.0",
    workers = 4,
    routes  = [
        "POST   /api/login"     :: handle_login,
        "GET    /api/user"      :: handle_user,
        "DELETE /api/users"     :: handle_delete,
        "OPTIONS *"             :: handle_options,
        "*"                     :: handle_404
    ]
};
```

| Field     | Type         | Required | Default       | Description                               |
|-----------|--------------|----------|---------------|-------------------------------------------|
| `port`    | `i`          | Yes      | —             | Port number (1–65535)                     |
| `host`    | `s`          | No       | `"127.0.0.1"` | `"0.0.0.0"` = all interfaces              |
| `workers` | `i`          | No       | `1`           | Fibers per request                        |
| `routes`  | route list   | Yes      | —             | Checked top-to-bottom, first-match wins   |

> [!NOTE]
> `serve:` is a **terminal statement**. No code after this directive will execute.
> Wildcard `*` matches any method or path — always place it last.

### Handler Fibers

Every handler must be a fiber with the signature `fiber name(json: req -> json)`:

```xcx
fiber handle_health(json: req -> json) {
    yield net.respond(200, <<< {"status": "ok"} >>>);
};
```

A handler that does not call `yield net.respond(...)` results in an automatic `500 Internal Server Error`.

### net.respond()

```xcx
yield net.respond(200, my_json);
yield net.respond(201, my_json, ["Location" :: "/users/42"]);
yield net.respond(204, <<< {} >>>);
yield net.respond(404, <<< {"error": "not found"} >>>);
```

| Parameter | Type        | Required | Description                          |
|-----------|-------------|----------|--------------------------------------|
| status    | `i`         | Yes      | HTTP status code                     |
| body      | `json \| s` | Yes      | JSON object or raw string; use `<<< {} >>>` for empty JSON |
| headers   | `map:s<->s` | No       | Additional response headers          |

### CORS and Preflight

By default, the XCX engine provides automatic CORS support by adding these headers if they are missing from the response:
- `Access-Control-Allow-Origin: *`
- `Access-Control-Allow-Methods: GET, POST, OPTIONS, DELETE, PATCH`
- `Access-Control-Allow-Headers: Content-Type, Authorization, X-CSRF-TOKEN`

> [!TIP]
> **Recommended Practice**: While the engine provides defaults, it is **highly recommended** to explicitly declare these headers in your code. This ensures your application remains predictable across different environments and clearly documents its security policy.

For preflight (`OPTIONS`) support, use a dedicated handler to explicitly set allowed methods and headers:

```xcx
fiber handle_options(json: req -> json) {
    yield net.respond(204, <<< {} >>>, [
        {
            "Access-Control-Allow-Methods": "GET, POST, DELETE, PATCH, OPTIONS",
            "Access-Control-Allow-Headers": "Content-Type, Authorization, X-CSRF-TOKEN"
        }
    ]);
};
```

### Concurrent Requests

Fibers allow overlapping I/O:

```xcx
fiber fetch(s: url -> json) {
    yield net.get(url);
};

fiber:json: f1 = fetch("https://api.example.com/users");
fiber:json: f2 = fetch("https://api.example.com/posts");
json: r1 = f1.next();
json: r2 = f2.next();
```

### Security Constraints

| Constraint                      | Behavior                                           |
|---------------------------------|----------------------------------------------------|
| `localhost` / `127.0.0.1`       | Allowed by default                                 |
| `169.254.x.x` (link-local)      | `halt.fatal` — SSRF protection                     |
| `10.x`, `172.16.x`, `192.168.x` | Blocked in production mode                         |
| `file://` URLs                  | `halt.fatal`                                       |
| Max response body size          | 10 MB                                              |
| Max incoming request body       | 10 MB — returns 413 without invoking the handler   |
