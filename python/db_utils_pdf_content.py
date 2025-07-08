import os
import re
from typing import Optional
from functools import lru_cache

import pandas as pd  # type: ignore
import numpy as np  # type: ignore
from sqlalchemy import create_engine, text  # type: ignore
from sqlalchemy.engine import Engine  # type: ignore
from sklearn.preprocessing import MultiLabelBinarizer  # type: ignore

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

def extract_lot_section(pdf_text: str) -> str:
    """Extract the section starting with heading '5 Lot' and subsequent 5.x.x content.
    
    Parameters
    ----------
    pdf_text : str
        The full PDF text content.
        
    Returns
    -------
    str
        The extracted section text, or empty string if no section found.
    """
    if not pdf_text:
        return ""
        
    # Pattern to match "5 Lot" heading and subsequent 5.x.x sections
    # This regex looks for:
    # - "5 Lot" (case insensitive) optionally followed by other text
    # - Everything until the next major section (6.x.x or similar)
    pattern = r'(?i)(?:^|\n)\s*5\s+lot.*?(?=(?:^|\n)\s*6\s+|\Z)'
    
    matches = re.findall(pattern, pdf_text, re.DOTALL | re.MULTILINE)
    
    if matches:
        # Return the first match, cleaned up
        return matches[0].strip()
    
    # If no "5 Lot" found, try to find any 5.x.x sections
    pattern_5x = r'(?i)(?:^|\n)\s*5\.\d+.*?(?=(?:^|\n)\s*6\.\d+|\Z)'
    matches_5x = re.findall(pattern_5x, pdf_text, re.DOTALL | re.MULTILINE)
    
    if matches_5x:
        # Join all 5.x.x sections
        return '\n'.join(match.strip() for match in matches_5x)
    
    return ""

def load_tender_records_with_pdf_content(
    engine: Optional[Engine] = None, 
    include_unlabelled: bool = True,
    extract_lot_section_only: bool = True,
    pdf_content_only: bool = False
) -> pd.DataFrame:
    """Load tender_records joined with pdf_content data.

    Parameters
    ----------
    engine : sqlalchemy.engine.Engine, optional
        Existing engine. If *None*, a new one is constructed with :func:`get_db_engine`.
    include_unlabelled : bool, default True
        If *False*, rows where `bid` is NULL will be filtered out so that the
        returned frame can be used immediately for supervised training.
    extract_lot_section_only : bool, default True
        If *True*, extract only the "5 Lot" section and 5.x.x content from pdf_text.
        If *False*, use the full pdf_text.
    pdf_content_only : bool, default False
        If *True*, only include records that have actual PDF text content.

    Returns
    -------
    pandas.DataFrame
        DataFrame with columns from tender_records plus pdf_text, detected_codes, and codes_count
    """
    if engine is None:
        engine = get_db_engine()

    query = """
    SELECT 
        tr.*,
        pc.pdf_text,
        pc.detected_codes,
        pc.codes_count,
        pc.extraction_timestamp,
        pc.processing_status
    FROM tender_records tr
    LEFT JOIN pdf_content pc ON tr.resource_id = CAST(pc.resource_id AS BIGINT)
    """
    
    # Make sure to print the query for debugging
    print("SQL Query:", query)
    
    conditions = []
    if not include_unlabelled:
        conditions.append("tr.bid IS NOT NULL")
    
    if pdf_content_only:
        conditions.append("pc.pdf_text IS NOT NULL")
        conditions.append("LENGTH(TRIM(pc.pdf_text)) > 0")
    
    if conditions:
        query += " WHERE " + " AND ".join(conditions)

    df = pd.read_sql(text(query), con=engine)
    
    # Process pdf_text to extract lot section if requested
    if extract_lot_section_only:
        df['pdf_text'] = df['pdf_text'].apply(
            lambda x: extract_lot_section(x) if pd.notna(x) else ""
        )
    
    # Fill NaN values for PDF content columns
    df['pdf_text'].fillna("", inplace=True)
    df['codes_count'].fillna(0, inplace=True)
    df['detected_codes'] = df['detected_codes'].apply(
        lambda x: x if isinstance(x, list) else []
    )
    
    return df

def create_codes_onehot_encoding(df: pd.DataFrame) -> pd.DataFrame:
    """Create one-hot encoding for detected_codes column.
    
    Parameters
    ----------
    df : pd.DataFrame
        DataFrame with 'detected_codes' column containing lists of codes
        
    Returns
    -------
    pd.DataFrame
        DataFrame with additional one-hot encoded columns for each unique code
    """
    # Get all unique codes from detected_codes lists
    all_codes = []
    for codes_list in df['detected_codes']:
        if isinstance(codes_list, list):
            all_codes.extend(codes_list)
    
    # Create MultiLabelBinarizer for one-hot encoding
    mlb = MultiLabelBinarizer()
    
    # Transform detected_codes to one-hot encoded matrix
    codes_onehot = mlb.fit_transform(df['detected_codes'])
    
    # Create DataFrame with one-hot encoded columns
    codes_df = pd.DataFrame(
        codes_onehot, 
        columns=[f"code_{code}" for code in mlb.classes_],
        index=df.index
    )
    
    # Concatenate with original DataFrame
    result_df = pd.concat([df, codes_df], axis=1)
    
    return result_df

def get_available_codes(engine: Optional[Engine] = None) -> list[str]:
    """Get list of all unique codes detected in the pdf_content table.
    
    Parameters
    ----------
    engine : sqlalchemy.engine.Engine, optional
        Existing engine. If *None*, a new one is constructed with :func:`get_db_engine`.
        
    Returns
    -------
    list[str]
        List of all unique codes found in detected_codes arrays
    """
    if engine is None:
        engine = get_db_engine()
    
    query = """
    SELECT DISTINCT unnest(detected_codes) as code
    FROM pdf_content
    WHERE detected_codes IS NOT NULL
    ORDER BY code
    """
    
    result = pd.read_sql(text(query), con=engine)
    return result['code'].tolist()

def demo():
    """Quick test that shows the enhanced functionality with PDF content."""
    print("Loading tender records with PDF content...")
    df = load_tender_records_with_pdf_content()
    
    print(f"Total rows loaded: {len(df)}")
    print(f"Rows with PDF content: {df['pdf_text'].notna().sum()}")
    print(f"Rows with detected codes: {df['codes_count'].gt(0).sum()}")
    print(f"Average codes per document: {df['codes_count'].mean():.2f}")
    
    # Show distribution of bid labels
    print("\nBid label distribution:")
    print(df.groupby("bid").size())
    
    # Show sample of codes
    print("\nSample detected codes:")
    codes_with_content = df[df['codes_count'] > 0]['detected_codes'].head(5)
    for i, codes in enumerate(codes_with_content):
        print(f"  Document {i+1}: {codes}")
    
    # Show available codes
    print(f"\nTotal unique codes available: {len(get_available_codes())}")
    
    # Demo one-hot encoding
    print("\nCreating one-hot encoding for codes...")
    df_with_onehot = create_codes_onehot_encoding(df)
    code_columns = [col for col in df_with_onehot.columns if col.startswith('code_')]
    print(f"Created {len(code_columns)} one-hot encoded code columns")
    
    return df_with_onehot

if __name__ == "__main__":
    demo() 