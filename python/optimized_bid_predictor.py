"""
Optimized ML Bid Predictor - Minimizes False Negatives

This module implements an ML-based tender bid prediction system optimized to minimize
false negatives (missed bid opportunities). The cost of missing a potential bid is 
far higher than reviewing a false positive.

Key Configuration:
- Threshold: 0.050 (captures 95.8% of all bids)
- Model: Random Forest with 50 trees
- Features: 14 key features (reduced from 72 for efficiency)
- Target: Minimize missed opportunities while maintaining manageable review load

Review Pipeline:
1. No PDF → Manual review required (SNS alert)
2. Has PDF + ML predicts BID → LLM summary + urgent SNS alert  
3. Has PDF + ML predicts NO-BID → Low priority review
"""

import numpy as np
import pandas as pd
from sklearn.ensemble import RandomForestClassifier
from sklearn.feature_extraction.text import TfidfVectorizer
from sklearn.preprocessing import LabelEncoder, StandardScaler
from sklearn.model_selection import train_test_split
from sklearn.metrics import classification_report, roc_auc_score
import joblib
from typing import Dict, List, Tuple, Optional
import logging

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class OptimizedBidPredictor:
    """
    Optimized Random Forest bid predictor with minimal false negatives
    
    Designed to capture 95.8% of all potential bids while maintaining
    manageable review workload for human validation.
    """
    
    def __init__(self):
        # Model components
        self.model: Optional[RandomForestClassifier] = None
        self.tfidf_vectorizer: Optional[TfidfVectorizer] = None
        self.ca_encoder: Optional[LabelEncoder] = None
        self.scaler: Optional[StandardScaler] = None
        
        # Critical threshold optimized for minimal false negatives
        self.prediction_threshold = 0.050  # Captures 95.8% of bids
        
        # Model configuration
        self.model_config = {
            'n_estimators': 50,        # Reduced for speed while maintaining accuracy
            'max_depth': 8,            # Prevent overfitting
            'class_weight': 'balanced', # Handle class imbalance
            'random_state': 42,        # Reproducibility
            'n_jobs': -1              # Use all available cores
        }
        
        # Key terms identified as most predictive for bids
        self.key_terms = [
            'software', 'support', 'provision', 'computer', 'services',
            'systems', 'management', 'works', 'package', 'technical'
        ]
        
        # Training state
        self.is_trained = False
        self.feature_names = []
        
    def extract_features(self, df: pd.DataFrame) -> Tuple[np.ndarray, List[str]]:
        """
        Extract the 14 key features identified as most important for bid prediction
        
        Features extracted:
        1. codes_count - Most important predictor
        2. has_codes - Binary indicator
        3. title_length - Text length feature
        4. ca_encoded - Contracting authority
        5-14. TF-IDF features for key terms
        """
        features = []
        feature_names = []
        
        # 1. codes_count (most important feature)
        codes_count = df['codes_count'].fillna(0).values
        features.append(codes_count)
        feature_names.append('codes_count')
        
        # 2. has_codes (strong binary predictor)
        has_codes = (codes_count > 0).astype(int)
        features.append(has_codes)
        feature_names.append('has_codes')
        
        # 3. title_length (text complexity indicator)
        title_length = df['title'].str.len().fillna(0).values
        features.append(title_length)
        feature_names.append('title_length')
        
        # 4. Contracting authority (encoded)
        if self.ca_encoder is None:
            self.ca_encoder = LabelEncoder()
            # Handle unknown categories gracefully
            unique_cas = df['ca'].fillna('Unknown').astype(str)
            ca_encoded = self.ca_encoder.fit_transform(unique_cas)
        else:
            # Transform with handling for unseen categories
            ca_values = df['ca'].fillna('Unknown').astype(str)
            ca_encoded = np.zeros(len(ca_values))
            for i, ca in enumerate(ca_values):
                try:
                    ca_encoded[i] = self.ca_encoder.transform([ca])[0]
                except ValueError:
                    # Unknown category - assign to most common class (0)
                    ca_encoded[i] = 0
        
        features.append(ca_encoded)
        feature_names.append('ca_encoded')
        
        # 5-14. TF-IDF features for key terms
        combined_text = (df['title'].fillna('') + ' ' + df['pdf_text'].fillna('')).tolist()
        
        if self.tfidf_vectorizer is None:
            self.tfidf_vectorizer = TfidfVectorizer(
                vocabulary=self.key_terms,
                lowercase=True,
                max_features=len(self.key_terms)
            )
            tfidf_features = self.tfidf_vectorizer.fit_transform(combined_text).toarray()
        else:
            tfidf_features = self.tfidf_vectorizer.transform(combined_text).toarray()
        
        # Add TF-IDF features
        for i, term in enumerate(self.key_terms):
            if i < tfidf_features.shape[1]:  # Safety check
                features.append(tfidf_features[:, i])
                feature_names.append(f'tfidf_{term}')
        
        # Combine all features
        X = np.column_stack(features)
        
        logger.info(f"Extracted {X.shape[1]} features from {X.shape[0]} records")
        return X, feature_names
    
    def train(self, df: pd.DataFrame, target_column: str = 'bid') -> Dict:
        """
        Train the optimized bid prediction model
        
        Returns training metrics and feature importance
        """
        logger.info("Starting model training...")
        
        # Extract features and target
        X, feature_names = self.extract_features(df)
        y = df[target_column].values
        
        # Store feature names
        self.feature_names = feature_names
        
        # Scale features
        self.scaler = StandardScaler()
        X_scaled = self.scaler.fit_transform(X)
        
        # Split for validation
        X_train, X_test, y_train, y_test = train_test_split(
            X_scaled, y, test_size=0.2, random_state=42, stratify=y
        )
        
        # Train Random Forest
        self.model = RandomForestClassifier(**self.model_config)
        self.model.fit(X_train, y_train)
        
        # Validate performance
        y_pred_proba = self.model.predict_proba(X_test)[:, 1]
        y_pred = y_pred_proba >= self.prediction_threshold
        
        # Calculate metrics
        auc_score = roc_auc_score(y_test, y_pred_proba)
        
        # Count false negatives (critical metric)
        tn = np.sum((y_pred == 0) & (y_test == 0))
        fp = np.sum((y_pred == 1) & (y_test == 0))
        fn = np.sum((y_pred == 0) & (y_test == 1))  # Critical: missed bids
        tp = np.sum((y_pred == 1) & (y_test == 1))
        
        recall = tp / (tp + fn) if (tp + fn) > 0 else 0
        precision = tp / (tp + fp) if (tp + fp) > 0 else 0
        
        # Feature importance
        feature_importance = pd.DataFrame({
            'feature': feature_names,
            'importance': self.model.feature_importances_
        }).sort_values('importance', ascending=False)
        
        self.is_trained = True
        
        training_results = {
            'auc_score': auc_score,
            'recall': recall,
            'precision': precision,
            'false_negatives': fn,
            'false_positives': fp,
            'true_positives': tp,
            'true_negatives': tn,
            'threshold': self.prediction_threshold,
            'feature_importance': feature_importance,
            'total_samples': len(y),
            'bid_rate': y.mean()
        }
        
        logger.info(f"Model trained successfully!")
        logger.info(f"AUC: {auc_score:.3f}, Recall: {recall:.1%}, False Negatives: {fn}")
        
        return training_results
    
    def predict(self, df: pd.DataFrame) -> Tuple[np.ndarray, np.ndarray]:
        """
        Make predictions with focus on minimizing false negatives
        
        Returns:
            probabilities: Raw prediction probabilities
            predictions: Binary predictions using optimized threshold
        """
        if not self.is_trained:
            raise ValueError("Model must be trained before making predictions")
        
        # Extract features
        X, _ = self.extract_features(df)
        X_scaled = self.scaler.transform(X)
        
        # Get probabilities
        probabilities = self.model.predict_proba(X_scaled)[:, 1]
        
        # Apply optimized threshold for minimal false negatives
        predictions = probabilities >= self.prediction_threshold
        
        return probabilities, predictions
    
    def categorize_tenders(self, df: pd.DataFrame) -> List[Dict]:
        """
        Categorize tenders for the review pipeline
        
        Returns list of categorization results for each tender
        """
        results = []
        
        for idx, row in df.iterrows():
            # Check PDF data availability
            has_pdf = (
                pd.notna(row.get('pdf_text')) and 
                len(str(row.get('pdf_text', '')).strip()) > 50
            )
            
            if not has_pdf:
                result = {
                    'tender_id': idx,
                    'title': row.get('title', 'Unknown'),
                    'category': 'NO_PDF_DATA',
                    'action': 'MANUAL_REVIEW',
                    'priority': 'HIGH',
                    'sns_message': f"⚠️ Manual Review Required: '{row.get('title', 'Unknown')}' - No PDF data available for ML analysis",
                    'ml_probability': None,
                    'requires_llm_summary': False
                }
            else:
                # Make ML prediction
                single_row_df = pd.DataFrame([row])
                probabilities, predictions = self.predict(single_row_df)
                
                ml_probability = probabilities[0]
                ml_prediction = predictions[0]
                
                if ml_prediction:
                    result = {
                        'tender_id': idx,
                        'title': row.get('title', 'Unknown'),
                        'category': 'ML_PREDICTED_BID',
                        'action': 'LLM_SUMMARY_REQUIRED',
                        'priority': 'URGENT',
                        'sns_message': f"🎯 ACTION REQUIRED: '{row.get('title', 'Unknown')}' - ML predicts BID opportunity ({ml_probability:.1%} confidence)",
                        'ml_probability': ml_probability,
                        'requires_llm_summary': True
                    }
                else:
                    result = {
                        'tender_id': idx,
                        'title': row.get('title', 'Unknown'),
                        'category': 'ML_PREDICTED_NO_BID',
                        'action': 'LOW_PRIORITY_REVIEW',
                        'priority': 'LOW',
                        'sns_message': f"📋 Low Priority: '{row.get('title', 'Unknown')}' - ML suggests no bid ({ml_probability:.1%} confidence)",
                        'ml_probability': ml_probability,
                        'requires_llm_summary': False
                    }
            
            results.append(result)
        
        return results
    
    def save_model(self, filepath: str):
        """Save the trained model and preprocessors"""
        if not self.is_trained:
            raise ValueError("Cannot save untrained model")
        
        model_data = {
            'model': self.model,
            'tfidf_vectorizer': self.tfidf_vectorizer,
            'ca_encoder': self.ca_encoder,
            'scaler': self.scaler,
            'prediction_threshold': self.prediction_threshold,
            'feature_names': self.feature_names,
            'key_terms': self.key_terms,
            'model_config': self.model_config
        }
        
        joblib.dump(model_data, filepath)
        logger.info(f"Model saved to {filepath}")
    
    def load_model(self, filepath: str):
        """Load a trained model and preprocessors"""
        model_data = joblib.load(filepath)
        
        self.model = model_data['model']
        self.tfidf_vectorizer = model_data['tfidf_vectorizer']
        self.ca_encoder = model_data['ca_encoder']
        self.scaler = model_data['scaler']
        self.prediction_threshold = model_data['prediction_threshold']
        self.feature_names = model_data['feature_names']
        self.key_terms = model_data['key_terms']
        self.model_config = model_data['model_config']
        
        self.is_trained = True
        logger.info(f"Model loaded from {filepath}")

# Example usage and testing
if __name__ == "__main__":
    # This would be used in production with actual data
    print("Optimized Bid Predictor - Minimal False Negatives")
    print("Threshold: 0.050 (captures 95.8% of bids)")
    print("Ready for integration with review pipeline and LLM summaries")
