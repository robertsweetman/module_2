# Adv Sw Engineering module 2

## Notes:

This is the 'working code' version of Adv Sw Engineering module 2 about designing an ML model and training it. See the page https://robertsweetman.github.io/module_2/ for the current state of the submission document.

## TODO:

### Academic Requirements
- [ ] Bring in notes from word doc
- [ ] Read/make my own notes on the rubric
- [x] Bring in MDdoc setup and pipelines from Module 1
- [x] Pull data into PostGres DB
- [x] Store in S3? <- used RDS instead
- [x] TF s3 bucket and tf state
- [x] Define schema
- [x] Document connection and user settings
- [x] Use github secrets <- transitioned to AWS Secret store, for clarity
- [x] Add BID column for 'test data' category <- required for all data
- [x] Bid Y/N <- added manually 
- [ ] Look at serverless rust options maybe? - https://github.com/featurestoreorg/serverless-ml-course 
- [x] Print rubric
- [ ] Write submission document with respect to the rubric notes
- [ ] Rust model training <- used Python pandas instead, lack of time available to use Rust unfortunately
- [ ] any data cleaning required? <- Yes, see etenders.ipynb
- [ ] nulls? Null PDF's?
- [ ] what other data validation is needed?
- [ ] document all the data validation steps
- [ ] look at different comparisons
- [ ] write up why some were rejected
- [x] use Polars.rs pipeline <- used Pandas instead, see note ref Python above.
- [ ] Add AI summary part
- [ ] Bid confidence, based on training
- [ ] Reference back to the rubric for marking

### Processing Pipeline Architecture

#### 🔄 Current Pipeline Flow (WORKING)
```
postgres_dataload → (if has PDF) → pdf_processing_queue → ml_prediction_queue → (if bid worthy) → ai_summary_queue → sns_queue
                 → (if no PDF) → ai_summary_queue → sns_queue
```

#### 🎯 Data Flow Optimization Tasks
- [x] **CRITICAL: Fix queue message structure** - Pass full tender records through queues instead of just IDs
  - [x] Update `postgres_dataload` to send complete TenderRecord to queues (not just resource_id + pdf_url)
  - [x] Update `pdf_processing` to receive TenderRecord and add pdf_content field before forwarding
  - [x] Update `ml_bid_predictor` to receive TenderRecord (it already does this correctly!)
  - [x] Implement direct-to-ai-summary routing for records without PDFs in postgres_dataload
  
#### 🏗️ Missing Lambda Functions
- [ ] **Create AI Summary Lambda** 
  - [ ] Build new crate: `crates/ai_summary/`
  - [ ] Integrate with OpenAI/Claude API for tender summarization
  - [ ] Extract: Project costs, timelines, key contacts, requirements, supplier codes
  - [ ] Forward worthy tenders to SNS queue
  - [ ] Update processing_status in database
  
- [ ] **Create SNS Notification Lambda**
  - [ ] Build new crate: `crates/sns_notification/`
  - [ ] Send formatted notifications (email/Slack/webhook)
  - [ ] Mark records as 'notified' in database
  
#### 🗄️ Database Schema Updates
- [ ] **Add processing pipeline status tracking**
  - [ ] Add `processing_status` ENUM column: ('new', 'pdf_processing', 'ml_prediction', 'ai_summary', 'notified', 'rejected')
  - [ ] Add `processing_stage_timestamps` JSONB for tracking timing through pipeline
  - [ ] Add `rejection_reason` field for tracking why records were filtered out
  - [ ] Migration scripts for existing data

#### 🚀 Infrastructure Updates  
- [x] **Update Terraform for queue message routing**
  - [x] Add AI_SUMMARY_QUEUE_URL to postgres_dataload lambda
  - [x] Add ML_PREDICTION_QUEUE_URL to pdf_processing lambda  
- [ ] **Add missing queue triggers in Terraform**
  - [ ] ai_summary_queue → ai_summary lambda trigger
  - [ ] sns_queue → sns_notification lambda trigger
  - [ ] Update IAM policies for new lambdas
  
- [ ] **Update GitHub Actions**
  - [ ] Add ai_summary and sns_notification to build_lambdas.yml
  - [ ] Update deployment conditional logic

#### 🔍 Queue Message Structure Standardization
**Target message format for ALL queues:**
```json
{
  "resource_id": 12345,
  "title": "Software Development Services", 
  "contracting_authority": "HSE",
  "pdf_url": "https://example.com/tender.pdf",
  "pdf_content": "extracted text...",  // Added by pdf_processing
  "codes_count": 3,                    // Added by pdf_processing - count of detected codes
  "deadline": "2025-08-15T10:00:00",
  "value": "50000.00",
  "ml_prediction": {                    // Added by ml_bid_predictor
    "should_bid": true,
    "confidence": 0.85,
    "reasoning": "High IT relevance"
  },
  "processing_stage": "ai_summary",
  "... other tender fields ..."
}
```

### Using LLMs to Summarise Tenders (AI Summary Lambda)

#### 🤖 AI Summary Processing
- [ ] **Implement AI summary Lambda (priority: HIGH)**
  - [ ] Project Costs analysis and extraction
  - [ ] Project Timelines identification  
  - [ ] Key contacts extraction
  - [ ] Text Summary generation
  - [ ] Absolute requirements identification
  - [ ] Supplier codes verification
  - [ ] Integration with OpenAI/Claude API
  - [ ] Error handling for API failures
  - [ ] Cost optimization (token usage monitoring)

#### 📋 AI Summary Output Structure
```json
{
  "ai_summary": {
    "project_costs": {
      "budget_range": "€50,000 - €100,000",
      "payment_terms": "Monthly payments",
      "cost_breakdown": ["Development: 60%", "Testing: 20%", "Support: 20%"]
    },
    "timelines": {
      "project_duration": "6 months", 
      "key_milestones": ["Requirement gathering: Month 1", "Development: Month 2-4"],
      "deadline": "2025-08-15"
    },
    "contacts": {
      "procurement_officer": "John Smith <j.smith@hse.ie>",
      "technical_contact": "Jane Doe <jane.doe@hse.ie>"
    },
    "requirements": {
      "mandatory": ["ISO 27001 certification", "EU GDPR compliance"],
      "technical": ["REST APIs", "PostgreSQL database", "Cloud deployment"],
      "experience": ["3+ years healthcare IT", "Previous HSE work preferred"]
    },
    "supplier_codes": ["72000000", "72500000"],
    "bid_recommendation": {
      "recommend": true,
      "confidence": 0.92,
      "reasoning": "Strong technical fit, appropriate budget, realistic timeline"
    }
  }
}
```

#### 📨 Notification Pipeline  
- [ ] **SNS Integration**
  - [ ] Email notifications for high-confidence bids
  - [ ] Slack/Teams webhook integration
  - [ ] Dashboard updates
  - [ ] Weekly summary reports

### Next Steps Priority Order

#### 🚨 **IMMEDIATE (This Week)**
1. [x] ~~**Fix queue message structure**~~ ✅ **COMPLETED** - Updated postgres_dataload and pdf_processing to pass full records
2. **Create ai_summary Lambda** - Core business logic for tender analysis  
3. **Add processing_status column** - Database schema update for pipeline tracking

#### 📅 **SHORT TERM (Next 2 Weeks)**  
4. **Create SNS notification Lambda** - Complete the pipeline
5. **Update Terraform infrastructure** - Add missing queue triggers
6. **Update GitHub Actions** - Build and deploy new lambdas

#### 🎯 **MEDIUM TERM (Month)**
7. **Optimize AI costs** - Token usage monitoring and optimization
8. **Add monitoring & dashboards** - CloudWatch metrics and alarms
9. **Performance tuning** - Queue batch sizes, timeouts, concurrency limits

#### ✅ **COMPLETED**
- Queue message structure standardization (postgres_dataload → pdf_processing → ml_prediction)
- Full TenderRecord objects now flow through the pipeline
- Direct routing for non-PDF records to AI summary queue
- Terraform environment variable updates

## Features needed
- [x] ~~Fix the 'send summary to endpoint' part~~ ← **SOLVED: Using SQS → SNS pipeline**
  - [x] ~~Alternatively publish it there, pub/sub vs. @Version1 address (shrug)~~ ← **Using AWS SQS/SNS**
  - [x] ~~Investigate other options~~ ← **Architecture defined above**


### Structure: 

.gihub/workflows
 - build_lambdas            - build lambda's, upload to S3, notify new lambda version
 - terraform_plan_and_apply - run plan, apply following plan review
 - deploy_site              - deploy mdbook to github pages
 - generate_pdf             - generate a pdf for module submission from mdbook
aws_backend_bootstrap       - create S3 for tf state backend
aws_deploy_infrastructure   - deploys lambda's and other resources, uses s3 backend
crates
 - get_data                 - main postrgesql data-loading pipeline
 - postgres_dataload        - uses sqs to hand off pdf_url to pdf_processing
 - crates/pdf_processing    - processes pdf's from sqs
mcp-server                  - custom mcp server for interrogating the PostgreSQL RDS Db
mdbook                      - publish to github pages & also pdf export
python                      - jupyter notebook for data interrogation and cleaning

## Python analysis environment

A lightweight Python layer lives under `python/` for ad-hoc data exploration and model training.

1. Install dependencies (ideally in a virtualenv):

```bash
python -m venv .venv
source .venv/bin/activate
pip install -r python/requirements.txt
```

2. Set AWS credentials and (optionally) the Secrets Manager reference in your shell.  At minimum you need:

```bash
export AWS_REGION=...         # or your preferred region
export AWS_ACCESS_KEY_ID=AKIA...
export AWS_SECRET_ACCESS_KEY=...
export AWS_SECRETS_NAME=...
```

These variables are the first place boto3 looks for credentials [docs](https://boto3.amazonaws.com/v1/documentation/api/latest/guide/credentials.html#configuring-credentials).

3. Run a quick smoke test **from the project root**:

```bash
python -m python.db_utils
```

If you happen to be inside the `python/` directory, drop the package prefix:

```bash
cd python
python -m db_utils
```

Either command should print the number of rows in `tender_records` and a breakdown per **bid** label.

4. Launch Jupyter for interactive work:

```bash
jupyter lab
```

Then, in a notebook:

```python
from python.db_utils import load_tender_records  # or from db_utils if your CWD is python/

df = load_tender_records(include_unlabelled=False)
```

and proceed with feature engineering, train/test split, etc.

### Using AWS Secrets Manager

If you provisioned the `etenders_rds_credentials` secret via Terraform (see `aws_deploy_infrastructure/`), the helper will automatically pull it when `AWS_SECRETS_NAME` **or** `AWS_SECRETS_ARN` is set, so no plain-text connection strings are required.


