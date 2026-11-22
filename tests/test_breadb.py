import tempfile
import unittest
from pathlib import Path

from breadb import MiniDatabase


class MiniDatabaseTests(unittest.TestCase):
    def test_crud_and_filtering(self):
        db = MiniDatabase()
        users = db.create_table("users", ["name", "active"])

        first = users.insert({"name": "Ada", "active": True})
        users.insert({"name": "Grace", "active": False})

        self.assertEqual(first["id"], 1)
        self.assertEqual(users.get(1)["name"], "Ada")
        self.assertEqual(users.select({"active": False})[0]["name"], "Grace")
        self.assertEqual(users.update(1, {"name": "Augusta"})["name"], "Augusta")
        self.assertEqual(users.delete(1)["name"], "Augusta")
        self.assertIsNone(users.get(1))

    def test_save_and_load(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "nested" / "database.json"
            db = MiniDatabase(path)
            db.create_table("users", ["name"]).insert({"name": "Lin"})
            db.save()

            restored = MiniDatabase(path)
            self.assertEqual(restored.table("users").select()[0]["name"], "Lin")

    def test_validation(self):
        db = MiniDatabase()
        users = db.create_table("users", ["name"])
        with self.assertRaises(ValueError):
            users.insert({"active": True})
        with self.assertRaises(ValueError):
            users.insert({"id": 0, "name": "No"})
        users.insert({"id": 5, "name": "Unique"})
        with self.assertRaises(ValueError):
            users.insert({"id": 5, "name": "Duplicate"})


if __name__ == "__main__":
    unittest.main()
