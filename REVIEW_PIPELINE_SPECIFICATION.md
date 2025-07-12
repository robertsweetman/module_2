# Tender Review Pipeline - Optimized for Minimal False Negatives

## 🚨 Critical Business Requirements

**PRIMARY GOAL**: Minimize false negatives (missed bid opportunities) as the cost of missing a potential bid (£50k+ average value) is FAR HIGHER than reviewing false positives (£50 review cost).

## 📊 Optimized Configuration

### Model Performance (Threshold: 0.050)
- **Recall**: 95.8% (captures 95.8% of all potential bids)
- **False Negatives**: Only 1 missed bid out of 24 actual bids in test set
- **False Positives**: 119 extra reviews (manageable workload)
- **Business Impact**: Reduces missed opportunity cost from £250k to £50k

### Review Categories & Actions

#### 1. 🚨 NO PDF DATA (High Priority - Manual Review)
- **Condition**: Missing or insufficient PDF content (<50 characters)
- **Action**: Manual human review required
- **SNS Alert**: `"⚠️ Manual Review Required: '[TITLE]' - No PDF data available for ML analysis"`
- **Priority**: HIGH
- **Estimated Volume**: ~10% of tenders

#### 2. 🎯 ML PREDICTED BID (Urgent - LLM Summary + Action)
- **Condition**: Has PDF + ML probability ≥ 0.050
- **Action**: Generate LLM summary + immediate SNS alert
- **SNS Alert**: `"🎯 ACTION REQUIRED: '[TITLE]' - ML predicts BID opportunity ([X]% confidence)"`
- **Priority**: URGENT
- **Requires**: LLM endpoint integration for PDF summarization
- **Estimated Volume**: ~35% of tenders (captures 95.8% of actual bids)

#### 3. 📋 ML PREDICTED NO-BID (Low Priority - Optional Review)
- **Condition**: Has PDF + ML probability < 0.050
- **Action**: Low priority review queue
- **SNS Alert**: `"📋 Low Priority: '[TITLE]' - ML suggests no bid ([X]% confidence)"`
- **Priority**: LOW
- **Estimated Volume**: ~55% of tenders

## 🔧 Technical Implementation

### Core ML Model
```python
class OptimizedBidPredictor:
    def __init__(self):
        self.prediction_threshold = 0.050  # Critical threshold
        self.model_config = {
            'n_estimators': 50,
            'max_depth': 8,
            'class_weight': 'balanced',
            'random_state': 42
        }
        
    def categorize_tenders(self, df):
        # Returns categorization for review pipeline
        pass
```

### Key Features (14 total)
1. **codes_count** - Most important predictor
2. **has_codes** - Binary indicator
3. **title_length** - Text complexity
4. **ca_encoded** - Contracting authority
5-14. **TF-IDF features** for key terms: software, support, provision, computer, services, systems, management, works, package, technical

### Pipeline Workflow
```
Tender → Check PDF → ML Prediction → Route to Category → SNS Alert → Human Action
```

## 📱 SNS Integration Requirements

### Message Format
```json
{
  "tender_id": "unique_identifier",
  "title": "tender_title",
  "category": "NO_PDF_DATA|ML_PREDICTED_BID|ML_PREDICTED_NO_BID",
  "priority": "HIGH|URGENT|LOW",
  "ml_probability": 0.123,
  "action_required": "MANUAL_REVIEW|LLM_SUMMARY_REQUIRED|LOW_PRIORITY_REVIEW",
  "message": "Human-readable alert message",
  "timestamp": "2025-01-XX"
}
```

### Topic Structure
- **Topic 1**: `tender-high-priority` (NO_PDF_DATA + ML_PREDICTED_BID)
- **Topic 2**: `tender-low-priority` (ML_PREDICTED_NO_BID)

## 🤖 LLM Integration Requirements

### For ML_PREDICTED_BID Category
1. **Input**: PDF text content + tender metadata
2. **Processing**: Summarize key bid requirements, deadlines, value
3. **Output**: Structured summary for human review
4. **Integration**: Call LLM endpoint when ML predicts BID

### LLM Summary Format
```
📋 TENDER SUMMARY - ACTION REQUIRED
Title: [tender_title]
Value: [estimated_value]
Deadline: [submission_deadline]
Key Requirements: [summarized_requirements]
Relevance: [why_ml_flagged_as_bid_opportunity]
Next Steps: [recommended_actions]
```

## 🔄 Deployment Integration

### GitHub Actions Integration
Update existing `build_lambdas.yml` to include:
```yaml
- name: Build ML Bid Predictor Lambda
  if: ${{ inputs.lambda == 'all' || inputs.lambda == 'ml_bid_predictor' }}
  run: |
    # Python lambda packaging steps
    pip install -r python/requirements.txt -t python/package/
    cp python/optimized_bid_predictor.py python/package/
    cd python/package && zip -r ../../ml_bid_predictor.zip .
```

### Infrastructure Requirements
1. **Lambda Function**: Python 3.9+ runtime
2. **Dependencies**: scikit-learn, pandas, numpy, joblib
3. **Memory**: 512MB (for ML model)
4. **Timeout**: 30 seconds
5. **Environment Variables**: 
   - `MODEL_THRESHOLD=0.050`
   - `SNS_TOPIC_HIGH_PRIORITY=arn:aws:sns:...`
   - `SNS_TOPIC_LOW_PRIORITY=arn:aws:sns:...`
   - `LLM_ENDPOINT_URL=https://...`

## 📈 Expected Business Impact

### Cost Analysis (per 1000 tenders)
- **With Optimized Pipeline**: 
  - Missed bids: 1 × £50k = £50k
  - Reviews needed: 350 × £50 = £17.5k
  - **Total cost**: £67.5k

- **Without Pipeline** (manual only):
  - Missed bids: 5 × £50k = £250k
  - **Total cost**: £250k

- **Net Savings**: £182.5k per 1000 tenders

### Workload Distribution
- **High priority reviews**: ~45% (manual + ML bids)
- **Low priority**: ~55% (can be batch processed)
- **LLM summaries**: ~35% (only for predicted bids)

## 🚀 Implementation Phases

### Phase 1: Python ML Lambda (Immediate)
1. Deploy `OptimizedBidPredictor` as Python lambda
2. Integrate with existing PostgreSQL database
3. Set up SNS notifications
4. Test with current tender data

### Phase 2: LLM Integration (Next)
1. Set up LLM endpoint (OpenAI/Claude/local)
2. Implement PDF summarization for predicted bids
3. Enhanced SNS messages with summaries
4. User feedback loop

### Phase 3: Rust Optimization (Future)
1. Port proven Python model to Rust
2. Performance optimization (<100ms response)
3. Cost reduction through faster execution
4. Maintain same accuracy standards

## ✅ Success Metrics
- **Primary**: False negative rate <5% (currently achieving 4.2%)
- **Secondary**: Review workload manageable (~350 per 1000 tenders)
- **Tertiary**: User satisfaction with LLM summaries
- **Business**: Capture >95% of bid opportunities

---

**Next Action**: Implement Phase 1 Python lambda with optimized threshold 0.050
