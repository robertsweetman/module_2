# Developing AI and ML apps

## Application development 

We're going to go through app development stages which map onto the Project Gantt chart in [Team Collaboration and Communication](./5_team_collaboration_and_communication.md)

### Requirements Gathering
Stakeholders, in this case Version 1's Sales Team for Ireland, would clearly love to avoid trawling through un-related tenders of no interest to an IT consultancy. However, there is a significant opportunity cost to missing relevant tenders. This translates into a solid requirement where we're trying to reduce workload BUT not at the expense of a huge false negative rate where the model elects to mark a tender as 'no-bid' when it might be hugely valuable. 

### Prototype

#### Data Gathering
We can automate data gathering, manipulation and storage by leveraging infrastructure as code (IaC) tools like Terraform. This brings up an AWS RDS PostgreSQL database in the cloud, uses AWS Lambdas (written in Rust for speed and cheapness) REF: to get the data and pipe the data into the database.

To avoid the "it works on my machine" problem we should use GitHub Actions to drive cloud deployment, Lambda updates and ultimately ML execution/testing/training. Using action runners means anyone in the team can update the application, following appropriate review of course.

#### Model Selection
Python's wide range of ML libraries, and Jupyter notebooks allow us to quickly iterate on a data-frame and most importantly validate what the model might be doing or missing. 

It's not simply a case of throwing some data at a model, getting a nice looking F1 score (or something else) and deploying it. We absolutely have to take into account the business requirements.

Here's where the key testing and investigation work around ML happens.

### Development
Following architectural design and planning this phase runs in 2 week sprints to fulfill user stories that deliver incremental value. This ensures that if any blockers do arise they're identified early and don't derail the entire project in the last weeks. 

A key section in here is testing that 'all' tender records can be ingested, and making sure there's schema checks around data ingestion. As we're using Rust for a lot of the data pipeline it's type and compiler checking add a lot of value here. REF: needed!

### Deployment
Before official 'go-live' everything runs in the cloud environment as a 'smoke test' but the results are private and highly scritinised by the development team. Once they're happy a go/no-go decision can be made in consultation with the stakeholders, based on the results from running the ML model and reviewing the tenders it's suggesting, as well as those it's rejected. 

We can extend the automation already used in the prototyping phase to actually deploy the production components, update the ML model, update various Lambda functions and add functionality like informing the sales team that a bid should be looked at. 

After the deployment is live we might find users have feedback or there might be updates needed to deal with un-forseen issues. Since the deployment pipeline is now automated this shouldn't prove too challenging.

## Model selection and methodologies

We're looking to make a decision ("bid on this or not?") so it seemed immediately obvious to start our investigation using a decision tree. However, there's a fundamental issue with that assumption because our main piece of data on which to answer this bid/no-bid question is the tender title, which is all text. Additionally our data is highly weighted towards 'no-bid' as an outcome which makes using decision trees problematic. (Otten, 2024)

Think of it this way, the text field is going to be turned into a large list of numbers with one dimension per possible word, most of which aren't useful so end up being zero.

In a decision tree every split asks about the value of a word but we're doing this on very sparse data where there's far more "mostly zero" features that don't impact on whether a tender should be considered a bid opportunity. This means that if a word appears only once it easily lets the tree recognise a single record. Additionally it will perform very well on data it's already seen but badly on new records and it won't know what to do with tender titles that contain words it hasn't seen before.

With this in minde we need to look at what's better at least for this type of data. We still have only two possible outcomes and linear regression can also cope with this as well as being better dealing with the short text strings we have in our 'tender title' field.

If we use a Logistic (linear) Regression instead, every text feature is treated independently and there's no branching that the model might memorise. Also, having a lot of 'non-interesting' words doesn't have a massive impact as well as being better able to handle words it's not seen before. You get enough of a signal from words it does know about as opposed to a decision tree where if something doesn't appear in a tree 'branch' it fails to generate a score at all. This approach copes a lot better with previously unseen titles.

We will see this tendency for decision trees to over-fit in the analysis later.



