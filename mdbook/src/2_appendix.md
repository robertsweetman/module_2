# Appendix

There's a large GitHub repo with many moving parts associated with this project.

module_2 repo
* https://github.com/robertsweetman/module_2
* Repository for all the code used in this project
  * aws_backend_bootstrap
    * set up S3 backend state bucket for terraform
  * aws_deploy_infrastructure 
    * deploy all AWS resources using infrastructure as code
  * crates - rust based AWS Lambda code for actually running the pipeline
    * get_data
      * get the etenders data
      * format it correctly with the correct types
      * store content in an AWS RDS PostgreSQL database
      * if there's a PDF for the tender, get that as well
      * if there's a PDF, count how many IT related codes there are in the PDF text
      * record the text, number of codes and what they are if there is a PDF, add these to the PostgreSQL database
    * pdf_processing (see note 1)
    * postgres_dataload (see note 1)
  * mc-server
    * wrote a (minimally functional) MCP server to show other ways that data in the AWS RDS Postgresql db could be queried
  * mdbook 
    * source for PDF creation for the submission
      * also hosts the content on https://robertsweetman.github.io/module_2/
  * python 
    * db_utils.py make the database content available as a dataframe
    * etenders.ipynb - see below

etenders.ipynb 
* https://github.com/robertsweetman/module_2/blob/main/python/etenders.ipynb
* Initial investigative ML model workbook, sets out all the learning and discovery steps as well as model validation & hyper-parameter tuning

baseline_text_models.ipynb
* https://github.com/robertsweetman/module_2/blob/main/python/baseline_text_models.ipynb
* Checking out various text parameterization methods

hybrid_text_model_main.ipynb
* https://github.com/robertsweetman/module_2/blob/main/python/hybrid_text_model_main.ipynb
* Final checking and hyper-parameter tuning on training data

Note 1: postgres_dataload and pdf_processing Lambda functions were an attempt to use AWS Simple Queue Service (SQS) to asyncronously handle the 'pdf_processing' part of the data ingest pipeline. The idea being that we could add other POST(s) to the queue for a different lambda to process and keep the separation of concerns whereby one Lambda function executes one task. 

However, running out of time this was abandoned as an approach as it wasn't possible to properly test whether ALL pdf_processing requests were being handled correctly when pulled from SQS. Ideally this would be the way to do things and should be the goal, with a bit more experience of queueing systems!
