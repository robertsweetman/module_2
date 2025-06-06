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


