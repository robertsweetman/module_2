# Adv Sw Engineering module 2

## Notes:

Print the rubric sheet

## TODO:

- [ ] Bring in notes from word doc
- [ ] Read/make my own notes on the rubric
- [ ] Bring in MDdoc setup and pipelines from Module 1
- [ ] Pull data into PostGres DB
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


These 4 all need creating

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


