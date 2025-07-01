from __future__ import annotations

"""Utility helpers for loading and pre-processing the tender data for ML experiments.

The functions defined here should be import-ed by notebooks and scripts so we avoid
copy-pasting the same boiler-plate across multiple files.
"""

from typing import Optional

import pandas as pd  # type: ignore

try:
    from . import db_utils
except ImportError:
    import db_utils

# ---------------------------------------------------------------------------
# Public helpers
# ---------------------------------------------------------------------------


def load_clean_dataframe(*, labelled_only: bool = True) -> pd.DataFrame:
    """Return a tidy ``pandas.DataFrame`` ready for modelling.

    Columns returned: ``title``, ``ca``, ``procedure``, ``pdf_url`` and ``bid``.
    If *labelled_only* is *True* the function filters out rows where the label
    ``bid`` is NULL so the resulting frame can be used directly for supervised
    training/cross-validation.
    """
    # Fetch raw frame from the database via the existing helper.
    df = db_utils.load_tender_records(include_unlabelled=not labelled_only)

    # Select only the columns we care about for text-only baselines.
    keep_cols = [
        "title",       # short free-text string we will vectorise
        "ca",          # contracting authority (categorical)
        "procedure",   # procedure type (categorical)
        "pdf_url",     # link to PDF (presence/absence is a useful flag)
        "bid",         # ground-truth label 1/0
    ]
    df = df[keep_cols].copy()

    # Replace missing categorical values with empty string so that scikit-learn
    # encoders do not barf on NaNs.
    df["ca"].fillna("", inplace=True)
    df["procedure"].fillna("", inplace=True)
    # Make missing PDF links an empty string so string operations are safe.
    df["pdf_url"].fillna("", inplace=True)

    # Drop rows where *title* is missing entirely – they are useless for our
    # text-based experiments.
    df = df[df["title"].notna()]

    # If we requested *labelled_only* make sure the label is an integer.
    if labelled_only:
        df = df[df["bid"].notna()]
        df["bid"] = df["bid"].astype(int)

    return df


# Convenience alias expected by some older notebooks
get_baseline_dataframe = load_clean_dataframe 