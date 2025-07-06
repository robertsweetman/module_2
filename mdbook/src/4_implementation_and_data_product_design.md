# Implementation and Data Product Design

## Data Ingestion, Investigation and Prep

Each record consists of the following:-

**tender records** table
| Column name | Initial Type | Description                                    | Modified Type          | Drop |
|-------------|--------------|------------------------------------------------|------------------------|------|
| id          | integer      | row index in the database as records are added | -                      |  y   |
| title       | text         | title/subject of the tender                    | text                   |      |
| resource_id | text         | unique internal record number for the tender   | integer                |      |      
| ca          | text         | name of the contracting authority              | text                   |      |
| published   | text         | date tender was published                      | timestamp w/o timezone |  y   |
| deadline    | text         | deadline date for submission by contractor     | timestamp w/o timezone |  y   |
| procedure   | text         | what's the process of submitting a bid         | text                   |      |
| status      | text         | is the tender still open/closed and so on      | text                   |  y   |
| pdf_url     | text         | url for downloading the whole tender           | text                   |      |
| awarddate   | text         | when the tender was awarded                    | date                   |  y   |
| value       | text         | what is the tender worth?                      | numeric                |  y   |
| bid         | integer      | ML label: 1=bid, 0=no bid, NULL=unlabelled     | integer                |      |

All this has been pulled into PostgreSQL database for easy manipulation. See the tables above for the columns we're just going to drop since dates aren't at all relevant to our question. 

Then we can at least begin to consider which columns might help us answer the question -> "Should we respond to this tender?"

### Labelling the 'bid' column for Supervised learning

In order to be able to train a model on this data we've had to manually label 2000+ records with bid (1) or no bid (0), taking a supervised learning approach. Labelling 2000+ records took around 2 hours.

### Data Manipulation/Cleanup

#### id
This can be removed since `resource_id` is unique, we don't need another id. 

### title
This is the key field we're looking at training the model on to predict whether we should bid (bid column = 1) or not (bid column = 0). Happily EVERY record has a title field so we don't have to deal with null data in this case. 

### contracting authority (ca)
This might be relevant and as it has a finite number of possibilities we're going to one-hot encode this.

### procedure
This might be relevant and as it has a finite number of possibilities we're going to one-hot encode this.

### pdf_url
Not relevant for training so can be removed from the training data. We could _maybe_ turn this into a data point but as it is 25% of tenders don't have a PDF attached to them.

### value
Again nearly 50% of records don't have a value and making any assumptions about these (filling in values, adding a mean) would likely only introduce innacuracies. Conversely there might be multi-million pound values hidded in the PDF content. So with this in mind we can't rely on value as a feature of the tender data for ML training. 

## Decision Tree
Remember from [Model Selection and methodologies](./3_developing_ai_and_ml_apps.html#model-selection-and-methodologies) 


## Ethical Considerations

Training ML models should pay attention to local laws & statutes (i.e. GDPR), ensure the data is used in an ethical way and aim for transparency when it comes to any decisions that are under-pinned by any sort of AI or ML training process.

Version 1 also has a AI Governance framework REF: that goes through a number of discovery steps to ensure directives around AI are adhered to, maintain compliance and ensure any ethical and social implications are made clear to all parties involved.

## Regulatory Compliance

Version 1 (V1) is already signed up to access the eTenders data and respond so there is no part of the proposed usage that prohibits it's use, other than onward transmission to third parties. 

That said, for this to be more than a proof of concept/demo it would be appropriate to obtain permission from the Office of Government Procurement (OGP) to do anything further. At this stage everything is being used for learning and study, not commercial application.

However, with this in mind, it's already been proposed internally within V1 to respond to the OGP's question about how AI/ML might help them manage the tender process, increase transparency and modernise how they publish tenders by sharing this project/proof of concept with them. 