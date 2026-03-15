#!/usr/bin/env python3
"""
migrate.py - Database migration runner for HA-VoIP.

Reads SQL migration files in order, tracks applied versions in the
schema_version table, and supports both SQLite and PostgreSQL.

Usage:
    python migrate.py apply   [--db-url URL]   Apply pending migrations
    python migrate.py rollback [--db-url URL]  Rollback the last migration
    python migrate.py status  [--db-url URL]   Show migration status

Environment:
    DATABASE_URL   Connection string (sqlite:///path or postgres://...)
"""

import argparse
import glob
import os
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

# ---------------------------------------------------------------------------
# Database abstraction
# ---------------------------------------------------------------------------

class DatabaseConnection:
    """Minimal abstraction over SQLite and PostgreSQL connections."""

    def __init__(self, db_url: str):
        self.db_url = db_url
        self.conn = None
        self.db_type = "sqlite"

        if db_url.startswith("postgres://") or db_url.startswith("postgresql://"):
            self.db_type = "postgres"
        elif db_url.startswith("sqlite:///"):
            self.db_type = "sqlite"
        elif db_url.endswith(".db") or db_url.endswith(".sqlite"):
            self.db_type = "sqlite"
            self.db_url = f"sqlite:///{db_url}"
        else:
            # Default to SQLite with the given path
            self.db_type = "sqlite"

    def connect(self):
        """Establish database connection."""
        if self.db_type == "postgres":
            try:
                import psycopg2
            except ImportError:
                print("ERROR: psycopg2 is required for PostgreSQL. Install with:")
                print("  pip install psycopg2-binary")
                sys.exit(1)
            self.conn = psycopg2.connect(self.db_url)
            self.conn.autocommit = False
        else:
            import sqlite3
            db_path = self.db_url.replace("sqlite:///", "")
            if not db_path or db_path == ":memory:":
                db_path = ":memory:"
            else:
                os.makedirs(os.path.dirname(os.path.abspath(db_path)), exist_ok=True)
            self.conn = sqlite3.connect(db_path)

    def close(self):
        """Close database connection."""
        if self.conn:
            self.conn.close()

    def execute(self, sql: str, params=None):
        """Execute a SQL statement."""
        cursor = self.conn.cursor()
        if params:
            cursor.execute(sql, params)
        else:
            cursor.execute(sql)
        return cursor

    def executescript(self, sql: str):
        """Execute a SQL script (multiple statements)."""
        if self.db_type == "sqlite":
            self.conn.executescript(sql)
        else:
            cursor = self.conn.cursor()
            cursor.execute(sql)
            self.conn.commit()

    def commit(self):
        """Commit the current transaction."""
        self.conn.commit()

    def rollback(self):
        """Rollback the current transaction."""
        self.conn.rollback()

    def fetchone(self, sql: str, params=None):
        """Execute and fetch one row."""
        cursor = self.execute(sql, params)
        return cursor.fetchone()

    def fetchall(self, sql: str, params=None):
        """Execute and fetch all rows."""
        cursor = self.execute(sql, params)
        return cursor.fetchall()


# ---------------------------------------------------------------------------
# Migration logic
# ---------------------------------------------------------------------------

MIGRATIONS_DIR = Path(__file__).parent


def ensure_schema_version_table(db: DatabaseConnection):
    """Create the schema_version table if it does not exist."""
    db.execute("""
        CREATE TABLE IF NOT EXISTS schema_version (
            version     INTEGER PRIMARY KEY,
            applied_at  TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    """)
    db.commit()


def get_applied_versions(db: DatabaseConnection) -> set:
    """Return the set of already-applied migration versions."""
    try:
        rows = db.fetchall("SELECT version FROM schema_version ORDER BY version")
        return {row[0] for row in rows}
    except Exception:
        return set()


def discover_migrations() -> list:
    """
    Find all SQL migration files in the migrations directory.
    Returns a sorted list of (version, filepath) tuples.
    """
    pattern = os.path.join(MIGRATIONS_DIR, "[0-9]*.sql")
    files = glob.glob(pattern)
    migrations = []

    for filepath in files:
        filename = os.path.basename(filepath)
        match = re.match(r"^(\d+)", filename)
        if match:
            version = int(match.group(1))
            migrations.append((version, filepath))

    migrations.sort(key=lambda x: x[0])
    return migrations


def cmd_apply(db: DatabaseConnection):
    """Apply all pending migrations in order."""
    ensure_schema_version_table(db)
    applied = get_applied_versions(db)
    migrations = discover_migrations()
    pending = [(v, f) for v, f in migrations if v not in applied]

    if not pending:
        print("All migrations are already applied. Nothing to do.")
        return

    print(f"Found {len(pending)} pending migration(s):")
    for version, filepath in pending:
        filename = os.path.basename(filepath)
        print(f"  [{version:03d}] {filename}")

    for version, filepath in pending:
        filename = os.path.basename(filepath)
        print(f"\n==> Applying migration {version:03d}: {filename}")

        with open(filepath, "r", encoding="utf-8") as f:
            sql = f.read()

        try:
            # Remove any INSERT INTO schema_version from the SQL since we
            # handle version tracking ourselves.
            cleaned_sql = re.sub(
                r"INSERT\s+INTO\s+schema_version.*?;",
                "-- (version tracked by migrate.py)",
                sql,
                flags=re.IGNORECASE | re.DOTALL,
            )

            db.executescript(cleaned_sql)

            # Record the version
            now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")
            if db.db_type == "postgres":
                db.execute(
                    "INSERT INTO schema_version (version, applied_at) VALUES (%s, %s)",
                    (version, now),
                )
            else:
                db.execute(
                    "INSERT INTO schema_version (version, applied_at) VALUES (?, ?)",
                    (version, now),
                )
            db.commit()
            print(f"    Applied successfully.")

        except Exception as e:
            db.rollback()
            print(f"    ERROR: {e}")
            print(f"    Migration {version:03d} failed. Stopping.")
            sys.exit(1)

    print(f"\nAll {len(pending)} migration(s) applied successfully.")


def cmd_rollback(db: DatabaseConnection):
    """Rollback the most recently applied migration."""
    ensure_schema_version_table(db)
    applied = get_applied_versions(db)

    if not applied:
        print("No migrations to rollback.")
        return

    latest_version = max(applied)
    migrations = discover_migrations()
    migration_file = None

    for version, filepath in migrations:
        if version == latest_version:
            migration_file = filepath
            break

    print(f"==> Rolling back migration {latest_version:03d}")

    if migration_file:
        # Read the migration to identify tables created
        with open(migration_file, "r", encoding="utf-8") as f:
            sql = f.read()

        # Extract CREATE TABLE statements and drop them in reverse
        tables = re.findall(
            r"CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)",
            sql,
            re.IGNORECASE,
        )

        if tables:
            print(f"    Tables to drop: {', '.join(reversed(tables))}")
            for table in reversed(tables):
                if table == "schema_version":
                    continue
                try:
                    db.execute(f"DROP TABLE IF EXISTS {table}")
                    print(f"    Dropped table: {table}")
                except Exception as e:
                    print(f"    Warning: Could not drop {table}: {e}")

    # Remove version record
    if db.db_type == "postgres":
        db.execute("DELETE FROM schema_version WHERE version = %s", (latest_version,))
    else:
        db.execute("DELETE FROM schema_version WHERE version = ?", (latest_version,))

    db.commit()
    print(f"    Rollback of migration {latest_version:03d} complete.")


def cmd_status(db: DatabaseConnection):
    """Show the status of all migrations."""
    ensure_schema_version_table(db)
    applied = get_applied_versions(db)
    migrations = discover_migrations()

    # Get applied_at timestamps
    applied_info = {}
    try:
        rows = db.fetchall("SELECT version, applied_at FROM schema_version ORDER BY version")
        for row in rows:
            applied_info[row[0]] = row[1]
    except Exception:
        pass

    print(f"Database type: {db.db_type}")
    print(f"Database URL:  {db.db_url}")
    print(f"Migrations directory: {MIGRATIONS_DIR}")
    print()

    if not migrations:
        print("No migration files found.")
        return

    print(f"{'Version':<10} {'Status':<12} {'Applied At':<22} {'File'}")
    print("-" * 80)

    for version, filepath in migrations:
        filename = os.path.basename(filepath)
        if version in applied:
            status = "applied"
            applied_at = str(applied_info.get(version, "unknown"))
        else:
            status = "pending"
            applied_at = "-"
        print(f"{version:<10} {status:<12} {applied_at:<22} {filename}")

    applied_count = sum(1 for v, _ in migrations if v in applied)
    pending_count = len(migrations) - applied_count
    print()
    print(f"Total: {len(migrations)} migration(s), {applied_count} applied, {pending_count} pending")


# ---------------------------------------------------------------------------
# CLI entry point
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="HA-VoIP database migration runner",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Commands:
  apply      Apply all pending migrations
  rollback   Rollback the last applied migration
  status     Show migration status

Examples:
  python migrate.py status
  python migrate.py apply --db-url sqlite:///voip.db
  python migrate.py apply --db-url postgres://user:pass@localhost:5432/havoip
  python migrate.py rollback
        """,
    )

    parser.add_argument(
        "command",
        choices=["apply", "rollback", "status"],
        help="Migration command to execute",
    )
    parser.add_argument(
        "--db-url",
        default=os.environ.get("DATABASE_URL", "sqlite:///ha-voip.db"),
        help="Database connection URL (default: $DATABASE_URL or sqlite:///ha-voip.db)",
    )

    args = parser.parse_args()

    db = DatabaseConnection(args.db_url)

    try:
        db.connect()
        print(f"Connected to {db.db_type} database.\n")

        if args.command == "apply":
            cmd_apply(db)
        elif args.command == "rollback":
            cmd_rollback(db)
        elif args.command == "status":
            cmd_status(db)

    except KeyboardInterrupt:
        print("\nInterrupted.")
        sys.exit(130)
    except Exception as e:
        print(f"ERROR: {e}")
        sys.exit(1)
    finally:
        db.close()


if __name__ == "__main__":
    main()
