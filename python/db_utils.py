import os
from typing import Optional
from functools import lru_cache

import pandas as pd  # type: ignore
from sqlalchemy import create_engine, text  # type: ignore
from sqlalchemy.engine import Engine  # type: ignore

# Try to load a local .env for developer convenience if python-dotenv is installed.
try:
    from dotenv import load_dotenv, find_dotenv  # type: ignore
    # Use *find_dotenv* so running scripts from sub-directories still locate the
    # project-level .env file.
    load_dotenv(find_dotenv(), override=False)
except ModuleNotFoundError:
    # No local .env file or dependency; silently continue.
    pass

try:
    import boto3  # type: ignore
    from botocore.exceptions import BotoCoreError, ClientError  # type: ignore
except ImportError:  # boto3 is optional; only required if AWS secrets are used
    boto3 = None  # type: ignore

AWS_SECRET_ENV_KEYS = ("AWS_SECRETS_NAME", "AWS_SECRETS_ARN")

@lru_cache(maxsize=1)
def _load_db_secret_from_aws() -> dict[str, str] | None:
    """Return DB credentials loaded from AWS Secrets Manager if configured.

    The function looks for either `AWS_SECRETS_NAME` or `AWS_SECRETS_ARN` in the
    environment.  The referenced secret must contain a JSON payload with at
    least the keys `host`, `port`, `username`, `password`, and `database`, OR
    a key `DATABASE_URL`.

    Returns *None* if no env var is set or boto3 is not available.
    """
    secret_ref = None
    for key in AWS_SECRET_ENV_KEYS:
        if os.getenv(key):
            secret_ref = os.getenv(key)
            break

    if secret_ref is None or boto3 is None:
        return None

    client = boto3.client("secretsmanager")
    try:
        resp = client.get_secret_value(SecretId=secret_ref)
    except (BotoCoreError, ClientError):
        return None

    secret_string = resp.get("SecretString")
    if not secret_string:
        return None

    try:
        import json  # stdlib

        data = json.loads(secret_string)
        if not isinstance(data, dict):
            return None
        return data  # type: ignore
    except json.JSONDecodeError:
        return None

_DEFAULT_QUERY = """
SELECT *
FROM tender_records
"""

def _build_connection_uri() -> str:
    """Return a PostgreSQL SQLAlchemy connection URI.

    Priority order:
    1. `DATABASE_URL` env var (already in SQLAlchemy-compatible form).
    2. AWS Secrets Manager (if `AWS_SECRETS_NAME` or `AWS_SECRETS_ARN` is set).
    3. Individual components: DB_HOST, DB_PORT, DB_NAME, DB_USER, DB_PASSWORD.
    """
    database_url = os.getenv("DATABASE_URL")
    if database_url:
        return database_url

    # Try AWS Secrets Manager
    secret = _load_db_secret_from_aws()
    if secret is not None:
        if "DATABASE_URL" in secret:
            return secret["DATABASE_URL"]
        host = secret.get("host", secret.get("hostname", "localhost"))
        port = secret.get("port", 5432)
        name = secret.get("database", secret.get("db_name", "postgres"))
        user = secret.get("username", secret.get("user", "postgres"))
        pwd = secret.get("password", secret.get("pwd", "postgres"))
        return f"postgresql+psycopg2://{user}:{pwd}@{host}:{port}/{name}"

    host = os.getenv("DB_HOST", "localhost")
    port = os.getenv("DB_PORT", "5432")
    name = os.getenv("DB_NAME", "postgres")
    user = os.getenv("DB_USER", "postgres")
    pwd = os.getenv("DB_PASSWORD", "postgres")

    return f"postgresql+psycopg2://{user}:{pwd}@{host}:{port}/{name}"

def get_db_engine() -> Engine:
    """Return a SQLAlchemy engine using env vars or a .env file (if present)."""
    connection_uri = _build_connection_uri()
    return create_engine(connection_uri)


def load_tender_records(engine: Optional[Engine] = None, include_unlabelled: bool = True) -> pd.DataFrame:
    """Load tender_records into a pandas DataFrame.

    Parameters
    ----------
    engine : sqlalchemy.engine.Engine, optional
        Existing engine. If *None*, a new one is constructed with :func:`get_db_engine`.
    include_unlabelled : bool, default True
        If *False*, rows where `bid` is NULL will be filtered out so that the
        returned frame can be used immediately for supervised training.

    Returns
    -------
    pandas.DataFrame
    """
    if engine is None:
        engine = get_db_engine()

    query = _DEFAULT_QUERY
    if not include_unlabelled:
        query += " WHERE bid IS NOT NULL"

    return pd.read_sql(text(query), con=engine)


def demo():
    """Quick test that a connection works and prints basic stats."""
    df = load_tender_records()
    print("Rows loaded:", len(df))
    print(df.groupby("bid").size())


if __name__ == "__main__":
    demo() 