from __future__ import annotations

"""Utility helpers for loading and pre-processing tender data with PDF content for ML experiments.

This module extends the functionality of prepare_data.py to include PDF content analysis,
detected codes processing, and lot section extraction for enhanced feature engineering.
"""

from typing import Optional

import pandas as pd  # type: ignore
import numpy as np  # type: ignore

try:
    from . import db_utils_pdf_content
except ImportError:
    import db_utils_pdf_content

# ---------------------------------------------------------------------------
# Public helpers
# ---------------------------------------------------------------------------

def load_clean_dataframe_with_pdf_content(
    *, 
    labelled_only: bool = True,
    extract_lot_section_only: bool = True,
    include_codes_onehot: bool = True,
    pdf_content_only: bool = False
) -> pd.DataFrame:
    """Return a tidy ``pandas.DataFrame`` with PDF content ready for modelling.

    Columns returned: ``title``, ``ca``, ``procedure``, ``pdf_url``, ``bid``,
    ``pdf_text``, ``codes_count``, and optionally one-hot encoded code columns.
    
    If *labelled_only* is *True* the function filters out rows where the label
    ``bid`` is NULL so the resulting frame can be used directly for supervised
    training/cross-validation.
    
    Parameters
    ----------
    labelled_only : bool, default True
        If *True*, filter out rows where `bid` is NULL.
    extract_lot_section_only : bool, default True
        If *True*, extract only the "5 Lot" section and 5.x.x content from pdf_text.
        If *False*, use the full pdf_text.
    include_codes_onehot : bool, default True
        If *True*, include one-hot encoded columns for detected codes.
    pdf_content_only : bool, default False
        If *True*, only include records that have actual PDF text content.
        
    Returns
    -------
    pandas.DataFrame
        Enhanced DataFrame with PDF content and code features
    """
    # Fetch enhanced frame from the database via the new helper.
    df = db_utils_pdf_content.load_tender_records_with_pdf_content(
        include_unlabelled=not labelled_only,
        extract_lot_section_only=extract_lot_section_only,
        pdf_content_only=pdf_content_only
    )

    # Select columns we care about for text-based baselines with PDF content
    keep_cols = [
        "title",           # short free-text string we will vectorise
        "ca",              # contracting authority (categorical)
        "procedure",       # procedure type (categorical)
        "pdf_url",         # link to PDF (presence/absence is a useful flag)
        "bid",             # ground-truth label 1/0
        "pdf_text",        # extracted PDF text (potentially lot section only)
        "codes_count",     # number of detected codes
        "detected_codes",  # list of detected codes
    ]
    
    # Add PDF content metadata columns if available
    if "extraction_timestamp" in df.columns:
        keep_cols.append("extraction_timestamp")
    if "processing_status" in df.columns:
        keep_cols.append("processing_status")
    
    df = df[keep_cols].copy()

    # Replace missing categorical values with empty string so that scikit-learn
    # encoders do not barf on NaNs.
    df["ca"].fillna("", inplace=True)
    df["procedure"].fillna("", inplace=True)
    
    # Make missing PDF links an empty string so string operations are safe.
    df["pdf_url"].fillna("", inplace=True)
    
    # Ensure PDF content columns are properly formatted
    df["pdf_text"].fillna("", inplace=True)
    df["codes_count"].fillna(0, inplace=True)
    df["detected_codes"] = df["detected_codes"].apply(
        lambda x: x if isinstance(x, list) else []
    )

    # Drop rows where *title* is missing entirely – they are useless for our
    # text-based experiments.
    df = df[df["title"].notna()]

    # If we requested *labelled_only* make sure the label is an integer.
    if labelled_only:
        df = df[df["bid"].notna()]
        df["bid"] = df["bid"].astype(int)

    # Create one-hot encoding for detected codes if requested
    if include_codes_onehot:
        df = db_utils_pdf_content.create_codes_onehot_encoding(df)

    return df

def load_enhanced_features_dataframe(
    *, 
    labelled_only: bool = True,
    extract_lot_section_only: bool = True,
    include_codes_onehot: bool = True,
    include_text_features: bool = True,
    pdf_content_only: bool = False
) -> pd.DataFrame:
    """Return a DataFrame with enhanced features for advanced modeling.
    
    This function creates additional engineered features from the PDF content
    and text data that can be useful for machine learning models.
    
    Parameters
    ----------
    labelled_only : bool, default True
        If *True*, filter out rows where `bid` is NULL.
    extract_lot_section_only : bool, default True
        If *True*, extract only the "5 Lot" section and 5.x.x content from pdf_text.
    include_codes_onehot : bool, default True
        If *True*, include one-hot encoded columns for detected codes.
    include_text_features : bool, default True
        If *True*, include engineered text features like length, word counts, etc.
    pdf_content_only : bool, default False
        If *True*, only include records that have actual PDF text content.
        
    Returns
    -------
    pandas.DataFrame
        DataFrame with enhanced features for modeling
    """
    # Load the base dataframe
    df = load_clean_dataframe_with_pdf_content(
        labelled_only=labelled_only,
        extract_lot_section_only=extract_lot_section_only,
        include_codes_onehot=include_codes_onehot,
        pdf_content_only=pdf_content_only
    )
    
    # Create additional features
    if include_text_features:
        df = _add_text_features(df)
    
    # Add PDF availability flag
    df["has_pdf"] = (df["pdf_url"].notna() & df["pdf_url"].str.strip().ne("")).astype(int)
    
    # Add PDF content availability flag
    df["has_pdf_content"] = (df["pdf_text"].notna() & df["pdf_text"].str.strip().ne("")).astype(int)
    
    # Add codes availability flag
    df["has_codes"] = (df["codes_count"] > 0).astype(int)
    
    return df

def _add_text_features(df: pd.DataFrame) -> pd.DataFrame:
    """Add engineered text features to the DataFrame.
    
    Parameters
    ----------
    df : pd.DataFrame
        Input DataFrame with text columns
        
    Returns
    -------
    pd.DataFrame
        DataFrame with additional text features
    """
    # Title features
    df["title_length"] = df["title"].str.len()
    df["title_word_count"] = df["title"].str.split().str.len()
    
    # PDF text features
    df["pdf_text_length"] = df["pdf_text"].str.len()
    df["pdf_text_word_count"] = df["pdf_text"].str.split().str.len()
    
    # Ratio of PDF text to title
    df["pdf_to_title_ratio"] = df["pdf_text_length"] / (df["title_length"] + 1)  # +1 to avoid division by zero
    
    # Code density (codes per 1000 characters of PDF text)
    df["code_density"] = df["codes_count"] / (df["pdf_text_length"] / 1000 + 1)
    
    return df

def create_modeling_dataset(
    *, 
    labelled_only: bool = True,
    extract_lot_section_only: bool = True,
    include_codes_onehot: bool = True,
    min_pdf_text_length: int = 10,
    min_codes_for_analysis: int = 0,
    pdf_content_only: bool = True
) -> pd.DataFrame:
    """Create a clean dataset optimized for machine learning modeling.
    
    This function applies additional filtering and preprocessing steps to create
    a dataset that's ready for direct use in ML pipelines.
    
    Parameters
    ----------
    labelled_only : bool, default True
        If *True*, filter out rows where `bid` is NULL.
    extract_lot_section_only : bool, default True
        If *True*, extract only the "5 Lot" section and 5.x.x content from pdf_text.
    include_codes_onehot : bool, default True
        If *True*, include one-hot encoded columns for detected codes.
    min_pdf_text_length : int, default 10
        Minimum length of PDF text to include in the dataset.
    min_codes_for_analysis : int, default 0
        Minimum number of codes required for inclusion in analysis.
    pdf_content_only : bool, default True
        If *True*, only include records that have actual PDF text content.
        
    Returns
    -------
    pandas.DataFrame
        Clean dataset ready for modeling
    """
    # Load enhanced features
    df = load_enhanced_features_dataframe(
        labelled_only=labelled_only,
        extract_lot_section_only=extract_lot_section_only,
        include_codes_onehot=include_codes_onehot,
        include_text_features=True,
        pdf_content_only=pdf_content_only
    )
    
    # Apply quality filters
    if min_pdf_text_length > 0:
        df = df[df["pdf_text_length"] >= min_pdf_text_length]
    
    if min_codes_for_analysis > 0:
        df = df[df["codes_count"] >= min_codes_for_analysis]
    
    # Remove rows with missing essential text data
    df = df[df["title"].notna() & df["title"].str.strip().ne("")]
    
    # Reset index after filtering
    df = df.reset_index(drop=True)
    
    return df

def get_code_statistics(df: Optional[pd.DataFrame] = None) -> pd.DataFrame:
    """Get statistics about detected codes in the dataset.
    
    Parameters
    ----------
    df : pd.DataFrame, optional
        DataFrame to analyze. If None, loads fresh data.
        
    Returns
    -------
    pd.DataFrame
        Statistics about code frequency and distribution
    """
    if df is None:
        df = load_clean_dataframe_with_pdf_content(
            labelled_only=False,
            include_codes_onehot=False
        )
    
    # Type assertion to help linter understand df is not None after the check above
    assert df is not None
    
    # Get all codes from detected_codes lists
    all_codes = []
    for codes_list in df['detected_codes']:
        if isinstance(codes_list, list):
            all_codes.extend(codes_list)
    
    # Create frequency statistics
    code_counts = pd.Series(all_codes).value_counts()
    
    # Calculate additional statistics
    stats_df = pd.DataFrame({
        'code': code_counts.index,
        'frequency': code_counts.values,
        'percentage': (code_counts.values / len(df)) * 100
    })
    
    return stats_df

def demo():
    """Quick demonstration of the enhanced PDF content functionality."""
    print("=== Enhanced PDF Content Data Loading Demo ===")
    
    # Load basic dataset
    print("\n1. Loading basic dataset with PDF content...")
    df_basic = load_clean_dataframe_with_pdf_content()
    print(f"   Loaded {len(df_basic)} rows")
    print(f"   Columns: {list(df_basic.columns)}")
    
    # Show PDF content statistics
    print(f"\n2. PDF Content Statistics:")
    print(f"   Rows with PDF content: {df_basic['has_pdf_content'].sum()}")
    print(f"   Rows with detected codes: {df_basic['has_codes'].sum()}")
    print(f"   Average codes per document: {df_basic['codes_count'].mean():.2f}")
    
    # Load enhanced features dataset
    print("\n3. Loading enhanced features dataset...")
    df_enhanced = load_enhanced_features_dataframe()
    print(f"   Loaded {len(df_enhanced)} rows")
    print(f"   Total columns: {len(df_enhanced.columns)}")
    
    # Show feature statistics
    text_features = ['title_length', 'title_word_count', 'pdf_text_length', 
                     'pdf_text_word_count', 'pdf_to_title_ratio', 'code_density']
    if all(col in df_enhanced.columns for col in text_features):
        print(f"\n4. Text Feature Statistics:")
        print(df_enhanced[text_features].describe())
    
    # Show code statistics
    print("\n5. Code Statistics:")
    code_stats = get_code_statistics(df_enhanced)
    print(f"   Total unique codes: {len(code_stats)}")
    print(f"   Top 10 most frequent codes:")
    print(code_stats.head(10))
    
    # Show one-hot encoded code columns
    code_columns = [col for col in df_enhanced.columns if col.startswith('code_')]
    print(f"\n6. One-hot encoded code columns: {len(code_columns)}")
    if code_columns:
        print(f"   Sample code columns: {code_columns[:5]}")
    
    # Create modeling dataset
    print("\n7. Creating modeling dataset...")
    df_modeling = create_modeling_dataset(
        min_pdf_text_length=50,
        min_codes_for_analysis=1
    )
    print(f"   Modeling dataset: {len(df_modeling)} rows")
    
    # Show label distribution
    print(f"\n8. Label Distribution in modeling dataset:")
    print(df_modeling['bid'].value_counts())
    
    return df_modeling

# Convenience aliases expected by some older notebooks
get_baseline_dataframe_with_pdf = load_clean_dataframe_with_pdf_content
get_enhanced_dataframe = load_enhanced_features_dataframe 