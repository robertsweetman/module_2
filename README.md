# Adv Sw Engineering module 2

## Notes:

This is the 'working code' version of Adv Sw Engineering module 2 about designing an ML model and training it. See the page https://robertsweetman.github.io/module_2/ for the current state of the submission document.

## TODO:

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
- [x] Bid Y/N <- added manuall 
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

## Using LLM's to summarise the tender
- [ ] AI summary code
 - [ ] Project Costs, deployment and running 
 - [ ] Project Timelines
 - [ ] Key contacts
 - [ ] Text Summary
 - [ ] Absolute requirements
 - [ ] Supplier codes
- [ ] Write up process documentation

## Features needed
- [ ] Fix the 'send summary to endpoint' part
  - [ ] Alternatively publish it there, pub/sub vs. @Version1 address (shrug)
  - [ ] Investigate other options


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


