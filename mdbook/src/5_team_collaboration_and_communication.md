# Team Collaboration and Communication plan

## Collaboration
Modern software development now sits on top of a whole tool chain to surface work, encourage transparency and foster communication. In this type of project we'd keep all the code in source control (Git), all the tasks in Jira and run a daily agile-type stand-up to share progress or air any blockers. 

There's usually a chat app like Slack or Teams for quick threads to tackle particular issues or problems.

Outside the feature development being carried out by developers there would be a Team Lead, nominally set with tasking individuals with work as well as acting as an arbiter on technical direction. Possibly there would be a Software Architect fulfilling this role but if it's a smaller project this might be un-necessary. 

The Team Lead would also likely inteface to the Project Manager who would have overall responsibility for reporting outwards as to the state of the development, especially in relation to time to completion. Their role involves understanding the true state of the project and communicating this to stake-holders. Expectation management is key here, especially if something un-expected comes up and there _might_ be a possible delay. 

You could easily use dashboards to broadcast the current project state, identify key milestones and plot progress along a gantt chart (see below)

<style>
  /* Default for dark themes - white text */
  .mermaid text {
    fill: white !important;
  }
  .mermaid .taskText, 
  .mermaid .sectionTitle, 
  .mermaid .grid text, 
  .mermaid .tickText,
  .mermaid .titleText,
  .mermaid .labelText,
  .mermaid .loopText,
  .mermaid .actor text {
    fill: white !important;
  }
  
  /* Handle mdBook light and rust themes - black text */
  html.light .mermaid text,
  html.light .mermaid .taskText,
  html.light .mermaid .sectionTitle,
  html.light .mermaid .grid text,
  html.light .mermaid .tickText,
  html.light .mermaid .titleText,
  html.light .mermaid .labelText,
  html.light .mermaid .loopText,
  html.light .mermaid .actor text,
  html.rust .mermaid text,
  html.rust .mermaid .taskText,
  html.rust .mermaid .sectionTitle,
  html.rust .mermaid .grid text,
  html.rust .mermaid .tickText,
  html.rust .mermaid .titleText,
  html.rust .mermaid .labelText,
  html.rust .mermaid .loopText,
  html.rust .mermaid .actor text {
    fill: black !important;
  }
  
  /* Ensure dark themes have white text */
  html.navy .mermaid text,
  html.navy .mermaid .taskText,
  html.navy .mermaid .sectionTitle,
  html.navy .mermaid .grid text,
  html.navy .mermaid .tickText,
  html.navy .mermaid .titleText,
  html.navy .mermaid .labelText,
  html.navy .mermaid .loopText,
  html.navy .mermaid .actor text,
  html.ayu .mermaid text,
  html.ayu .mermaid .taskText,
  html.ayu .mermaid .sectionTitle,
  html.ayu .mermaid .grid text,
  html.ayu .mermaid .tickText,
  html.ayu .mermaid .titleText,
  html.ayu .mermaid .labelText,
  html.ayu .mermaid .loopText,
  html.ayu .mermaid .actor text,
  html.coal .mermaid text,
  html.coal .mermaid .taskText,
  html.coal .mermaid .sectionTitle,
  html.coal .mermaid .grid text,
  html.coal .mermaid .tickText,
  html.coal .mermaid .titleText,
  html.coal .mermaid .labelText,
  html.coal .mermaid .loopText,
  html.coal .mermaid .actor text {
    fill: white !important;
  }
  
  /* Ensure links and other specific elements have correct colors in light themes */
  html.light .mermaid .flowchart-link,
  html.rust .mermaid .flowchart-link {
    stroke: #333 !important;
  }
  
  /* Ensure links have correct colors in dark themes */
  html.navy .mermaid .flowchart-link,
  html.ayu .mermaid .flowchart-link,
  html.coal .mermaid .flowchart-link {
    stroke: #ccc !important;
  }
  
  /* Additional styles for better visibility in all themes */
  .mermaid .grid path {
    stroke-opacity: 0.5;
  }
  .mermaid .today {
    stroke-width: 2px;
  }
</style>

```mermaid
gantt
    title Prototype
    dateFormat YYYY-MM-DD
    axisFormat %b '%y
    tickInterval 1month

    Stakeholders Discussion :a1, 2025-07-01, 2w
    Data Gathering         :a2, after a1, 3w
    Prototype              :a3, after a2, 5w
    Stakeholders Review :crit, a4, after a3, 1w
    Go/No-Go Decision       :crit, milestone, a5, after a4, 0d
```

```mermaid
gantt
    title Development
    dateFormat YYYY-MM-DD
    axisFormat %b '%y
    tickInterval 1month

    Architecture :a1, 2025-10-01, 2w
    Planning :crit, a2, 2025-10-01, 2w
    Sprint 1 :a3, after a2, 2w
    Sprint 2 :a4, after a3, 2w
    Sprint 3 :a5, after a4, 2w
    Sprint 4 :a6, after a5, 2w
```

```mermaid
gantt
    title Deployment
    dateFormat YYYY-MM-DD
    axisFormat %b '%y
    tickInterval 1month

    IaC deployment :a1, 2026-01-01, 1w
    Smoke Test :a2, after a1, 3w
    Go/No-Go Live :milestone, crit, a3, after a2, 0d
    Live Trial :a4, after a3, 2w
    Gather User Feedback :a5, after a4, 2w
    Update Model :crit, a6, after a5, 2w
```

There are stage gates following prototype delivery and after the model has been deployed (smoke test) for a number of weeks. This first gate gives stakeholders the opportunity to see how the application would work in theory. The second gate is for developers to check that the solution actually works in practice before 'go-live'. 

Then, once it's been running for two to four weeks updates can be made based on live data and user feedback. Maybe there's some part of the performance that needs tweaking?

## Sharing Best Practices
From my experience within Version 1 the primary way that best practices are shared is via 'lunch and learn' sessions. It's always challenging to pull people's attention from client work so recorded sessions make the information more accessible. 

Keeping these up-to-date might mean creating your own internal news letter, blog or wiki that updates the same topic when new or better ways of doing things come to light. Making this content public also massivley boosts the organisations visibility and credibility in the wider world.

