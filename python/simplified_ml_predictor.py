"""
ML Bid Predictor - Simplified Implementation
Based on the findings from enhanced_pdf_data_exploration.ipynb

This module implements the Random Forest model with the most important features
identified in our analysis for predicting bid recommendations.
"""

import pandas as pd
import numpy as np
import joblib
from sklearn.ensemble import RandomForestClassifier
from sklearn.feature_extraction.text import TfidfVectorizer
from sklearn.preprocessing import LabelEncoder, StandardScaler
import re
import logging
from typing import Dict, List, Tuple, Optional
import os

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class SimplifiedMLBidPredictor:
    """
    Simplified ML Bid Predictor focusing on the most important features
    identified in our analysis. Designed to be easily portable to Rust.
    """
    
    def __init__(self):
        self.model = None
        self.tfidf_vectorizer = None
        self.ca_encoder = None
        self.scaler = None
        self.is_trained = False
        
        # Key TF-IDF terms identified as most important
        self.key_terms = [
            'software', 'support', 'provision', 'computer', 'services',
            'systems', 'management', 'works', 'package', 'single',
            'technical', 'internet', 'framework', 'supplier'
        ]
        
        # Prediction threshold optimized for F1-score
        self.prediction_threshold = 0.210
    
    def extract_features(self, df: pd.DataFrame) -> np.ndarray:
        """
        Extract the key features identified in our analysis.
        
        Features (in order of importance):
        1. codes_count - Number of detected codes
        2. has_codes - Boolean: whether PDF has codes
        3. title_length - Length of tender title
        4. ca_encoded - Contracting authority (encoded)
        5. Key TF-IDF terms for important words
        """
        features = []
        feature_names = []
        
        # 1. Codes count (most important feature)
        codes_count = df['codes_count'].fillna(0).values
        features.append(codes_count)
        feature_names.append('codes_count')
        
        # 2. Has codes (binary)
        has_codes = (codes_count > 0).astype(int)
        features.append(has_codes)
        feature_names.append('has_codes')
        
        # 3. Title length
        title_length = df['title'].str.len().values
        features.append(title_length)
        feature_names.append('title_length')
        
        # 4. PDF text length
        pdf_text_length = df['pdf_text_length'].fillna(0).values
        features.append(pdf_text_length)
        feature_names.append('pdf_text_length')
        
        # 5. Contracting Authority (encoded)
        if self.ca_encoder is None:
            self.ca_encoder = LabelEncoder()
            ca_encoded = self.ca_encoder.fit_transform(df['ca'].fillna('Unknown'))
        else:
            # Handle unseen categories
            ca_values = df['ca'].fillna('Unknown')
            ca_encoded = []
            for ca in ca_values:
                if ca in self.ca_encoder.classes_:
                    ca_encoded.append(self.ca_encoder.transform([ca])[0])
                else:
                    ca_encoded.append(-1)  # Unknown category
            ca_encoded = np.array(ca_encoded)
        
        features.append(ca_encoded)
        feature_names.append('ca_encoded')
        
        # 6. Key TF-IDF features
        combined_text = []
        for i in range(len(df)):
            title = df.iloc[i]['title']
            pdf_text = df.iloc[i]['pdf_text'] if pd.notna(df.iloc[i]['pdf_text']) else ''
            combined_text.append(f"{title} {pdf_text}")
        
        if self.tfidf_vectorizer is None:
            # Create simplified TF-IDF with only key terms
            self.tfidf_vectorizer = TfidfVectorizer(
                vocabulary=self.key_terms,
                lowercase=True,
                stop_words='english'
            )
            tfidf_features = self.tfidf_vectorizer.fit_transform(combined_text).toarray()
        else:
            tfidf_features = self.tfidf_vectorizer.transform(combined_text).toarray()
        
        # Add TF-IDF features
        for i, term in enumerate(self.key_terms):
            features.append(tfidf_features[:, i])
            feature_names.append(f'tfidf_{term}')
        
        # Combine all features
        X = np.column_stack(features)
        
        # Scale features if scaler is available
        if self.scaler is not None:
            X = self.scaler.transform(X)
        
        return X, feature_names
    
    def train(self, df: pd.DataFrame) -> Dict:
        """
        Train the simplified Random Forest model.
        """
        logger.info("Training simplified ML bid predictor...")
        
        # Extract features
        X, feature_names = self.extract_features(df)
        y = df['bid'].values
        
        # Scale features
        self.scaler = StandardScaler()
        X_scaled = self.scaler.fit_transform(X)
        
        # Train Random Forest
        self.model = RandomForestClassifier(
            n_estimators=100,
            max_depth=10,
            min_samples_split=5,
            min_samples_leaf=2,
            class_weight='balanced',  # Handle class imbalance
            random_state=42
        )
        
        self.model.fit(X_scaled, y)
        self.is_trained = True
        
        # Calculate feature importance
        importances = self.model.feature_importances_
        feature_importance = {
            name: importance 
            for name, importance in zip(feature_names, importances)
        }
        
        # Sort by importance
        sorted_features = sorted(feature_importance.items(), key=lambda x: x[1], reverse=True)
        
        logger.info("Model trained successfully!")
        logger.info("Top 10 feature importances:")
        for i, (feature, importance) in enumerate(sorted_features[:10]):
            logger.info(f"  {i+1:2d}. {feature}: {importance:.4f}")
        
        return {
            'feature_importance': feature_importance,
            'n_features': len(feature_names),
            'n_samples': len(y),
            'class_distribution': np.bincount(y).tolist()
        }
    
    def predict(self, df: pd.DataFrame) -> List[Dict]:
        """
        Make predictions on new data.
        """
        if not self.is_trained:
            raise ValueError("Model must be trained before making predictions")
        
        # Extract features
        X, _ = self.extract_features(df)
        
        # Make predictions
        probabilities = self.model.predict_proba(X)[:, 1]  # Probability of bid
        binary_predictions = probabilities >= self.prediction_threshold
        
        # Calculate confidence (distance from threshold)
        confidence = np.where(
            binary_predictions,
            probabilities,
            1 - probabilities
        )
        
        # Create results
        results = []
        for i in range(len(df)):
            results.append({
                'tender_id': df.iloc[i]['id'] if 'id' in df.columns else i,
                'probability': float(probabilities[i]),
                'prediction': bool(binary_predictions[i]),
                'confidence': float(confidence[i]),
                'threshold_used': self.prediction_threshold
            })
        
        return results
    
    def save_model(self, filepath: str):
        """Save the trained model and preprocessors."""
        if not self.is_trained:
            raise ValueError("No trained model to save")
        
        model_data = {
            'model': self.model,
            'tfidf_vectorizer': self.tfidf_vectorizer,
            'ca_encoder': self.ca_encoder,
            'scaler': self.scaler,
            'key_terms': self.key_terms,
            'prediction_threshold': self.prediction_threshold
        }
        
        joblib.dump(model_data, filepath)
        logger.info(f"Model saved to {filepath}")
    
    def load_model(self, filepath: str):
        """Load a pre-trained model and preprocessors."""
        model_data = joblib.load(filepath)
        
        self.model = model_data['model']
        self.tfidf_vectorizer = model_data['tfidf_vectorizer']
        self.ca_encoder = model_data['ca_encoder']
        self.scaler = model_data['scaler']
        self.key_terms = model_data['key_terms']
        self.prediction_threshold = model_data['prediction_threshold']
        self.is_trained = True
        
        logger.info(f"Model loaded from {filepath}")

# Example usage and testing
if __name__ == "__main__":
    # This would be used for testing the model
    print("SimplifiedMLBidPredictor - Ready for integration")
    print("Key features implemented:")
    print("- codes_count, has_codes, title_length, ca_encoded")
    print("- TF-IDF for key terms: software, support, provision, computer, services")
    print("- Random Forest with 100 trees")
    print("- Optimized threshold: 0.210")
    print("- Expected AUC: >0.90, Recall: >0.75")
