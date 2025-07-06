# Machine Learning and AI principals

## ML and AI in this use-case
Firstly we need to spend a bit of time differentiating between Machine Learning (ML) and AI since ML has been very much been run over by AI hype.

AI as offered by ChatGPT, Anthropic Claude etc. relies on exposure to huge amounts of data with the goal of developing a more general response to a much wider range of questions across different subjects. 

Machine learning preceeded AI, using past data, algorithms and pattern identification to predict some sort of future state given data that hadn't been seen before. This was usually within a specific realm or narrow topic and taking one of three general approaches - supervised, un-supervised and semi-supervised.

Supervised learning seems most appropriate as we're because by labelling the data we can teach the model what is important and have it apply that to recognising the difference between tenders to bid on and not to bid on. 

Since we're asking a simple question ("Is this tender suitable to make a bid on or not?") that isn't something we can assume will work with un-supervised learning because there's no inherent pattern to learn/discover. We're primarily going to rely on one feature (title in tender_records) which isn't sufficient for an un-supervised approach to work. 

Similarly, semi-supervised learning isn't appropriate yet because we only have labelled records. In six, twelve or twenty-four months it might be well worth taking the baseline supervised model and re-running the training. See the section TODO: ADD NEW FEATURES AND IMPROVEMENTS SECTION []() later in this proposal.

## Integration into existing digital solution lifecycle

One of the key aspects of software development that especially applies to ML is quick iterations to see what may or may not give a useful result. 

We can use jupyter notebooks and Python's ML libraries for this 'prototyping/experimenting' and analysis phase. This can site very easily within the larger project and quickly drive the 'ML' part of the solution.

Breaking the delivery down into two week sprints, setting stage goals, reviewing the work on a regular basis with the stakeholders are all part of modern software delivery that fits very well into this sort of ML integration/delivery project.

Developers can independently work on the ML component without relying on or becoming a bottleneck for other solution components like hosting, access, deployment etc. As long as they've got access to the data in some form they can make progress.



<!-- 

-->