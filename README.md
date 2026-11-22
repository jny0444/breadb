# breadb

`breadb` is a small in-memory database with optional JSON persistence.

```python
from breadb import MiniDatabase

db = MiniDatabase("data.json")
users = db.create_table("users", ["name", "active"])

user = users.insert({"name": "Ada", "active": True})
users.select({"active": True})
users.update(user["id"], {"active": False})
users.delete(user["id"])

db.save()
```

Without a path, `MiniDatabase` keeps data only in memory. Calling `MiniDatabase(path)`
loads an existing database from that path. Tables support `insert`, `select`, `get`,
`update`, and `delete`; every inserted row receives an integer `id`.