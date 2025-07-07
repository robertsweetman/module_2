# Introduction & Executive Summary

## Overview
The Irish Government releases a huge range of tenders daily via email. These range from bin collection, recruitment services, building managment all the way through to ones we are interested in - IT services/consulting contracts. 

Wading through this fire-host of information manually is time-consuming, inefficient and can lead to missed opportunities. 

By training an ML model on the tenders data we should be able to answer the question "Which ones should we 'actually' bid on?" as opposed to having someone wade through 50 new tender opportunities per day.

## How AI and ML integration impacts organisational change

As with any new technology a huge factor associated with it's introduction is trust.

Primarily, can users rely on what they're being told? Can we assemble a body of evidence in the training of tools like this to prove it will work as intended, save on 'drudge' filtering work and not negatively impact the business?

There are many instances where software has been used as a smokescreen for dodging accountability, the Post Office scandal (Lehrer, 2024) being the most egregious recent example. 

Adding AI increases the temptation to simply offload governance entirely to a "black box". Consumers are already abdicating responsibility for major life decisions to AI (‌Dupré, 2025) sometimes very much to their own detriment. Businesses may end up being no different if ethics aren't adhered to.

To address these issues we can take some concrete steps: -

* Making sure that the ML model's workings are explained simply (see Appendix for links to Jupyter Notebooks).

* Appointing an Owner specifically responsible for the behaviour of the AI/ML aspect of the solution. 

* Adding feedback loops and an update schedule so that, even after it's deployed, it can be re-trained and upgraded. 

## Scope and Objectives
The report outlines the various approaches to this problem from an ML/AI point of view, including possible training methods and models, data cleaning steps and most importantly whether the solution achieves is business objectives.

Up-front let's say we need to get a fairly high degree of accuracy but not at the expense of missing out possible multi-million euro tender opportunities. We need to keep an eye on the false negative rate while saving the cost of manual effort to filter out tenders we're not interested in.

## Projected Benefits 
Having ML and AI provide a 'first look' screening of tenders will save Sales teams time spent on initial screening which translates into focussing only on relevant bids and having more signal & less noise should reduce the chance of missing any opportunities. 
