import json
from copy import deepcopy
from pathlib import Path


class Table:
    def __init__(self, columns=None):
        self.columns = list(columns) if columns else None
        self.rows = []
        self._next_id = 1

    def insert(self, row):
        record = deepcopy(dict(row))
        if self.columns is not None:
            missing = [name for name in self.columns if name not in record]
            if missing:
                raise ValueError("missing required columns: " + ", ".join(missing))
            unknown = [name for name in record if name not in self.columns and name != "id"]
            if unknown:
                raise ValueError("unknown columns: " + ", ".join(unknown))
        if "id" not in record:
            record = {"id": self._next_id, **record}
        else:
            if not isinstance(record["id"], int) or record["id"] < 1:
                raise ValueError("id must be a positive integer")
        self._next_id = max(self._next_id, record["id"] + 1)
        self.rows.append(record)
        return deepcopy(record)

    def select(self, where=None):
        if where is None:
            return deepcopy(self.rows)
        return deepcopy([row for row in self.rows if all(row.get(k) == v for k, v in where.items())])

    def get(self, row_id):
        for row in self.rows:
            if row["id"] == row_id:
                return deepcopy(row)
        return None

    def update(self, row_id, changes):
        changes = dict(changes)
        if "id" in changes and changes["id"] != row_id:
            raise ValueError("row id cannot be changed")
        for index, row in enumerate(self.rows):
            if row["id"] == row_id:
                updated = {**row, **changes}
                self.rows[index] = updated
                return deepcopy(updated)
        return None

    def delete(self, row_id):
        for index, row in enumerate(self.rows):
            if row["id"] == row_id:
                return deepcopy(self.rows.pop(index))
        return None


class MiniDatabase:
    def __init__(self, path=None):
        self.path = Path(path) if path is not None else None
        self.tables = {}
        if self.path and self.path.exists():
            self.load()

    def create_table(self, name, columns=None):
        if name in self.tables:
            raise ValueError("table already exists: " + name)
        self.tables[name] = Table(columns)
        return self.tables[name]

    def drop_table(self, name):
        return self.tables.pop(name, None) is not None

    def table(self, name):
        if name not in self.tables:
            raise KeyError("unknown table: " + name)
        return self.tables[name]

    def save(self, path=None):
        destination = Path(path) if path is not None else self.path
        if destination is None:
            raise ValueError("a database path is required")
        destination.parent.mkdir(parents=True, exist_ok=True)
        data = {
            name: {"columns": table.columns,
                    "rows": table.rows,
                    "next_id": table._next_id}
            for name, table in self.tables.items()
        }
        destination.write_text(json.dumps(data, indent=2), encoding="utf-8")
        self.path = destination

    def load(self, path=None):
        source = Path(path) if path is not None else self.path
        if source is None:
            raise ValueError("a database path is required")
        data = json.loads(source.read_text(encoding="utf-8"))
        self.tables = {}
        for name, stored in data.items():
            table = Table(stored.get("columns"))
            table.rows = stored.get("rows", [])
            table._next_id = stored.get("next_id", 1)
            self.tables[name] = table
        self.path = source

    def __getitem__(self, name):
        return self.table(name)


# Friendly aliases for a small, straightforward public API.
Database = MiniDatabase
MiniDB = MiniDatabase

