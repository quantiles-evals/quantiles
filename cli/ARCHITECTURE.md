# `qt`'s Architecture

The Quantiles CLI, `qt`, keeps execution simple: your code runs locally, while `qt` handles durability and observability.

```
+--------------------------------------+
|            Your Script               |
|   (TypeScript / Python / Shell)      |
+--------------------+-----------------+
                     │
                     │  HTTP / JSON
                     ▼
+--------------------------------------+
|            Quantiles Server          |
+--------------------+-----------------+
                     │
                     │  SQLite
                     ▼
+------------------------------------------------+
|     .quantiles/quantiles.sqlite (local DB)     |
+--------------------+---------------------------+
                     │
                     ▼
+--------------------------------------+
|                 CLI                  |
|        (list, show, compare)         |
+--------------------------------------+
```

- **Server** owns durability decisions: step caching, run state, metrics
- **Client** (your script) owns code execution: the server never runs your logic
- **CLI** reads the same SQLite database the server writes to
