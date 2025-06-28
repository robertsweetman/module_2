# Adv Sw Engineering module 2

## Notes:

Print the rubric sheet

## TODO:

0. Bring in notes from word doc
  - Read/make my own notes on the rubric
  - Bring in MDdoc setup and pipelines from Module 1
1. Pull data into PostGres DB
  - Store in S3?
  - TF s3 bucket and tf state
  - Define schema
  - Document connection and user settings
  - Use github secrets
2. Add column for 'test data' category
  - Bid Y/N
  - Do this manually
3. Look at serverless rust options maybe?#
  - https://github.com/featurestoreorg/serverless-ml-course 
4. Print rubric
5. Write up rubric notes in submission document
6. Rust model training
  - any data cleaning required?
  - nulls? Null PDF's?
  - what other data validation is needed?
    - document all the data validation steps
  - look at different comparisons
  - write up why some were rejected
  - use Polars.rs pipeline
7. Add AI summary part
  - Bid confidence, based on training
  - AI summary
    - Costs
    - Timelines
    - Key contacts
    - Text Summary
    - Absolute requirements
    - Supplier codes
8. Write up process
9. Fix the 'send summary to endpoint' part
  - Alternatively publish it there, pub/sub vs. @Version1 address (shrug)
  - What other options
10. Reference back to the rubric for marking

### Structure: 

crates/aisummary
crates/datamanipulation
crates/modeltraining
crates/pushresults

These 4 all need creating

## Python analysis environment

A lightweight Python layer lives under `python/` for ad-hoc data exploration and model training.

1. Install dependencies (ideally in a virtualenv):

```bash
python -m venv .venv
source .venv/bin/activate
pip install -r python/requirements.txt
```

2. Supply database credentials.  Copy `env.example` to `.env` (ignored by git) and fill in the values that match your Amazon RDS instance:

```bash
cp env.example .env  # Optional for local dev; in CI set DATABASE_URL via secrets
# edit .env and set DB_HOST, DB_USER, ...
```

3. Run a quick smoke test:

```bash
python -m python.db_utils
```

This should print the number of rows in `tender_records` and a breakdown per **bid** label.

4. Launch Jupyter for interactive work:

```bash
jupyter lab
```

From a notebook you can now do:

```python
from python.db_utils import load_tender_records

df = load_tender_records(include_unlabelled=False)
```

and proceed with feature engineering, train/test split, etc.

### Using AWS Secrets Manager

If you provisioned the `etenders_rds_credentials` secret via Terraform, you can let the Python utilities fetch it automatically:

1. Ensure your local AWS credentials allow `secretsmanager:GetSecretValue` for that secret.
2. Export either the secret *name* or its ARN:

```bash
export AWS_SECRETS_NAME=etenders_rds_credentials   # or export AWS_SECRETS_ARN=arn:...
```

3. Run the smoke test again:

```bash
python -m python.db_utils
```

The helper will pull the JSON payload, construct a connection string, and connect—no `.env` file needed.


