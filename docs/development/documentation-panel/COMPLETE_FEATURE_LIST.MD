> This document will list all the features AutoDev have as i need to start documenting it

> For readability we will separate it based on the Panels AutoDev have and some more separation, regarding this i am just grouping based on the panel for easier listing, if you have a better method of grouping please do

# Personal Assistant (Chat Panel)
1. Explain the modes such ash:
1.1 Fast development, list all the tools it can use, the capabilities,
1.2 Review mode, same what is the feature here
1.3 Delegate Mode, same what is the feature here
2. Explain the LLM agnostic of PA that can be changed from the settings panel
3. The skills capabilities we can use to the PA
4. AutoDev.md
5. The session itself

# The AutoDev Fleet (The AutoDev Panel)
1. The fleet itself such as but not limited to (as i might have missed some stuff):
1.1 The fleet workers including supervisor and qc
1.2 How does the fleet operates and the features of the operation such as:
1.2.1 the supervisor breaking down tasks and semantic routing and worker affinity based on it
1.2.2 The QC Ephemeral agent and QC ROI
1.2.3 The workers explanation each and what does it do
1.2.4 The recipes (more detail on the recipes panel)
1.2.5 The worker request that can speak back to the supervisor in the event something happened
1.2.6 The cost and token tracker
1.2.7 The subagent spawning
1.2.8 The tools (briefly more on the tools exposed by the AutoDev section)
2. The fleet connection to the Agentic system of AIRS Forge
2.1 the fallback to its own rust engine so smooth experience

# The tools exposed by autodev
This section needs to explain the builtin_tools in src-tauri in a detailed pointer manner and explain the mcp server as well of the tools

# The IDE
1. IDE Explanation end to end on what it has and also the git visualization it has
2. The Annotation capabilities to PA and delegate and note

# The Recipes panel
the features the recipes panel have and what can the user do here

# The Visibility of the AutoDev
The logs panel feature it has so full visibility list and explain here

# independent feature (froms settings panel)
1. The demo mode to skip AIRS Forge entirely if needed
2. Default worker budger feature
3. Service and submodule tracker
4. The tools list user can see
5. The playwright capability of QC agent (should be in the autodev fleet section)
6. Provider selection for PA
7. Github MCP integration
8. Worker list panel and creation panel (should be in fleet section)
9. AutoDev system prompt section
10. Hooks detailed explanation
11. Updates section

# Graph
Explain graph in a damn detail manner, from what it can do, the feature list, the nodes visualization, the 3d city visualization etc

# Dashboard
Explain the dashboard feature and what we can see here
