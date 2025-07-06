# Recommendations and Conclusions

## Model Summary

While we've gotten off to a good start, the high False Negative rates will still have a detrimental impact (missed bids) versus the savings from not having to read through all tenders.

We need to spend more development time pulling in more data by deciding that all tenders without a PDF are reviewed manually. 

This leaves 75% of tenders with PDFs in the ML review pipeline but now offset by the fact that we should be able to increase the reliability becase we can add:-

* Relevant Tender codes from the PDF - use One Hot Encoding for these
* A count of the number of codes
* Additional amounts of text from the PDF's
  * Maybe only one section (summary, description etc.)
  * Maybe just take the first page

These steps would enrich the data and lead to much more reliable results. 

Since it's mainly a case of pulling this from our PostgreSQL db and running things again it wouldn't take a huge amount of development effort to add this. 

Sadly just using the tender title wasn't enough but we have a solid way forwards. 

## Further Improvements and evaluation

At some point an additional step would be to cross reference the bid/tenders database with "actual bids won" as well. This could add another layer to the entire excercise whereby it might also be possible to further increase the accuracy of our tender submission scanning and (with enough wins) possibly predict, based on past successes, whether a new tender is likely something we might win also.