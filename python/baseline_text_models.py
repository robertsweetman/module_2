# requires the following environment variables to be set:
# AWS_ACCESS_KEY_ID
# AWS_SECRET_ACCESS_KEY
# AWS_DEFAULT_REGION
# AWS_SECRETS_NAME

from __future__ import annotations

"""Train and evaluate simple text baselines (TF-IDF + LogReg/SVM, Hashing, etc.).

Run from the project root:

    python -m python.baseline_text_models

The script prints 5-fold cross-validated F1 scores for each model so you can
quickly compare their relative performance.
"""

from pathlib import Path

import numpy as np  # type: ignore
from sklearn.compose import ColumnTransformer  # type: ignore
from sklearn.feature_extraction.text import (  # type: ignore
    TfidfVectorizer,
    HashingVectorizer,
)
from sklearn.linear_model import LogisticRegression  # type: ignore
from sklearn.metrics import f1_score  # type: ignore
from sklearn.model_selection import StratifiedKFold, cross_val_score  # type: ignore
from sklearn.pipeline import Pipeline  # type: ignore
from sklearn.preprocessing import OneHotEncoder  # type: ignore
from sklearn.svm import LinearSVC  # type: ignore

from python.prepare_data import load_clean_dataframe  # type: ignore

RANDOM_STATE = 42
CV_SPLITS = 5


def _make_pipeline(vectoriser, classifier):
    """Return a scikit-learn ``Pipeline`` that joins text + categorical features."""
    categorical_features = ["ca", "procedure"]

    column_trans = ColumnTransformer(
        transformers=[
            ("text", vectoriser, "title"),
            ("cat", OneHotEncoder(handle_unknown="ignore"), categorical_features),
        ]
    )

    return Pipeline(
        [
            ("features", column_trans),
            ("clf", classifier),
        ]
    )


def _pretty_print(name: str, scores: np.ndarray) -> None:
    mean, std = scores.mean(), scores.std()
    print(f"{name:<25}  F1 = {mean:.3f} ± {std:.3f}  (n={len(scores)})")


def main() -> None:
    df = load_clean_dataframe(labelled_only=True)
    X = df[["title", "ca", "procedure"]]
    y = df["bid"].values

    cv = StratifiedKFold(n_splits=CV_SPLITS, shuffle=True, random_state=RANDOM_STATE)

    models: dict[str, Pipeline] = {
        "tfidf_logreg": _make_pipeline(
            TfidfVectorizer(ngram_range=(1, 2), min_df=3),
            LogisticRegression(
                max_iter=10_000,
                class_weight="balanced",
                n_jobs=-1,
                random_state=RANDOM_STATE,
            ),
        ),
        "tfidf_svm": _make_pipeline(
            TfidfVectorizer(ngram_range=(1, 2), min_df=3),
            LinearSVC(class_weight="balanced", random_state=RANDOM_STATE),
        ),
        "hashing_logreg": _make_pipeline(
            HashingVectorizer(analyzer="word", n_features=2**18),
            LogisticRegression(
                max_iter=10_000,
                class_weight="balanced",
                n_jobs=-1,
                random_state=RANDOM_STATE,
            ),
        ),
    }

    print("Evaluating baseline text models ({}-fold CV)\n".format(CV_SPLITS))

    for name, model in models.items():
        scores = cross_val_score(model, X, y, cv=cv, scoring="f1", n_jobs=-1)
        _pretty_print(name, scores)


if __name__ == "__main__":
    main() 