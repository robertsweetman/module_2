# Introduction & Executive Summary

## Overview
The Irish Government releases a huge range of tenders daily via email. These range from bin collection, general services, building managment all the way through anything a government may need to IT services/consulting contracts. Wading through this fire-host of information manually is time-consuming, inefficient and can lead to missing opportunities.

By training an ML model on the whole pipeline of possible tenders, with the aim of answering the question of "Which ones should we 'actually' bid on"?, we can find the tenders that should be responded to within the fire-hose of requests.

## How AI and ML integration impacts organisational change

As with any new technology introduction a huge factor associated with it's introduction is trust.

Primarily, can users rely on what they're being told? How can we assemble a body of evidence in the training of tools like this to prove to users the change will work in their favour?

Additionally, who is ultimately responsible for AI/ML's output and subsequent impact on the business? 

There are so many expamples already of IT in general being used as a smokescreen for dodging accountability, the Post Office scandal being the most egregious recent example. Adding AI into this mix increases complexity as well as the temptation to simply offload governance entirely to a "black box". Consumers are already abdicating responsibility for major life decisions, sometimes very much to their own detriment. Businesses may end up being no different if ethics aren't adhered to.

// TODO: add reference - Post Office scandal
// TODO: add reference - AI and Social media abuse

## Scope and Objectives
The report outlines the various approaches to this problem from an ML/AI point of view, including possible training methods and models,

## Projected Benefits 
Having ML and AI provide a 'first look' screening of tenders will deliver the following: -

* Remove time spent on intial screening
* Allow Sales teams to only focus on relevant bids
* Less noise, more signal reduces the chances of missing tenders 

We can also hand AI the job of creating a quickly scannable summary of tenders that the ML pipeline deems suitable. Something LLM's are very adept at. 

// TODO: follow on paragraph somewhere about what the Irish Govt have said to Paul about making the tender data more accessible/easier to parse and so on. Possibly use MCP for this!!

# Theoretical Approach

## ML and AI in this use-case
Firstly we need to spend a bit of time differentiating between ML and AI since recently they've been blended very much into one big bucket. At least ML has been somewhat subsumed by AI in the general public's consciousness.

Machine learning preceeded AI by using past data, algorithms and pattern identification to predict some sort of future state based on data it's not seen before. Usually within a specific realm or subject.

AI as offered by ChatGPT, Anthropic Claude etc. relies on exposure to huge amounts of data with the goal of developing a more general response to a much wider range of questions across different topics. 

While we can look at a vast array of ML approaches since we're looking to make a decision ("bid on this or not?") it seems immediately obvious to start our investigation using a decision tree. 

After that we'll have gotten a subset of tenders that we're interested in. At this point we're no longer looking to have the machine 'learn' anything about the tenders, we just want a human actionable summary. This is where AI comes in. 

* ML component - ask a question of a specific data set
  * Gather the data
  * Make the data palatable - pre-processing
  * Create a custom ML algorithm based on past data - model training
  * Supply a set of tender records that the new, trained, model deems should be responded to
* AI component - summarise this text

The key differentiation here is we're using ML to answer a specific question about a fairly simple data-set. With AI we're asking it to perform a general task for us in quite a hands-off way.

Arguable, given recent advancements, you could just remove the ML stage and throw everything at AI but this would prove more costly. Using ML to reduce the pool of tenders (using straightforward open-source tools) gives you better control over spending and allows you to tweak what's deemed acceptable to move to the AI summary step. 

## Integration into existing digital solution lifecycle

One of the key aspects of software development that especially applies to ML is quick iterations to see what may or may not give a useful result. Thankfully existing libraries allow fast iteration and just trying things out to see which approach might fit best. 

No-one wants to spend 6 months working on something that ultimately won't supply a result. 

Breaking the delivery down into two week sprints, setting stage goals, reviewing the work on a regular basis with the stakeholders are all part of modern software delivery that fits very well into this sort of ML integration/delivery project.

## Standards and Compliance

Version 1 (V1) is already signed up to access the eTenders data and respond so there is no part of the proposed usage that prohibits it's use, other than onward transmission to third parties. 

That said, for this to be more than a proof of concept/demo it would be appropriate to obtain permission from the Office of Government Procurement (OGP) to do anything further. At this stage everything is being used for learning and study, not commercial application.

However, with this in mind, it's already been proposed internally within V1 to respond to the OGP's question about how AI/ML might help them manage the tender process, increase transparency and modernise how they publish tenders by sharing this project/proof of concept with them.

# Tender Data Overview

At first glance the tender information looks pretty straightforward. Each record consists of the following:-


**tender records** table
| Column name | Type      | Description                                            | 
|-------------|-----------|--------------------------------------------------------|
| id          | integer   | row index in the database as records are added         |
| title       | text      | title/subject of the tender                            |
| resource_id | text      | unique internal record number for the tender           |                      
| ca          | text      | name of the contracting authority                      |
| published   | text      | date tender was published                              |
| deadline    | text      | deadline date for submission by contractor             |
| procedure   | text      | what's the process of submitting a bid                 |
| status      | text      | is the tender still open/closed and so on              |
| pdf_url     | text      | url for downloading the whole tender                   |
| awarddate   | text      | when the tender was awarded                            |
| value       | text      | what is the tender worth?                              |

**pdf content** table
| Column name | Type      | Description
|-------------|-----------|--------------------------------------------------------|
| resource_id | text      | unique internal record number of the tender            |
| pdf_text    | text      | content of the tender pdf                              |
| extraction_timestamp | timestamp with timezone | when was the pdf read into the db? |
| processing status | text | has the pdf been processed properly? |
| detected codes | text array | what codes have been identified in the tender? |
| codes_count | integer | how many codes were found? |

All this has been pulled into PostgreSQL database for easy manipulation. New tenders are pulled into the database on a daily basis as they're published, which includes parsing (reading and storing) pdf text as well since this contains some very useful additional context. 

The challenge with the data however is that 'most' if it appears in the original database as text, which is especially problematic when running queries. Since it's being dumped into a database we need to make some changes before we even think about it's "shape"...

1. Retroactively modify the db columns to change use better types for the columns
2. Change the Rust parsing code to match the types the db is now using

Then we can at least begin to consider which columns might help us answer the question -> "Should we respond to this tender?"

## Date Manipulation/Cleanup

This is the data and this is the problem - which drives model selection

Exploratory data analysis graphics

Evaluation metrics

## Model Selection

Which paradigms were considered
Supervised Learning - labelled data, use a training subset, why would this work?
Semi-Supervised learning - what this means and why not
Unsupervised and Reinforcement - what this means and why not

How do we conclude why Supervised is better/most suitable.

Use a decision tree 'cause this is most valid

# Workflow

# Testing and Debugging

# Model Selection

# Deployment

# Risk and Mitigation for AI - put this at the end?

# Business Impacts

# New Features and Continuous Improvements
