
# Extra Features & Refinements

**THIS SHOULD BE DONE AFTER PHASE 1 - 5 IS FINISHED**

## 0 (VERY IMPORTANT DO FIRST)
Create a `developer-guide.md` file that documents the project structure, first time installation steps, key components, and how to contribute.

## 1. Main Agent Improvements
- Replace compaction with a **proper memory system** referencing from `core-orchestrator` with three layer of memory system:
  - Episodic memory
  - Semantic memory
  - Procedural memory
- Specifically when using Google models, it always fails on reading file outside where the agent sits (claude or gpt works fine), need to:
    - Investigate the source of the issue
    - Refine the agent so gemini can works fine
    - Test with a smaller model like gemini-2.0-flash-lite

## 2. Main Agent LLM Provider Extra Features
1. Update pricing constants with actual values and ensue `cost today` is stored per day not when the app closes it resets back to 0.
2. Update the chat down right area that we can select model from there
3. Add thinking models and support with reasoning effort tunable from the settings
4. Add temperature and top-p parameters and each model can have an area we can select the values and saved properly in a configuration file similar to `model_registry.py`
5. Create a proper Ollama setup with local LLM providers using Ollama
6. Implement prompt caching for all LLM providers in Cloud mode
7. Add more models (o1, o3, gemini-2.0-flash, etc.)
8. Add model-specific context window validation

## 3. General Feature Improvements
1. `errors.rs` and its UI:
  - Improve the error categories, add more depth and details in error catching
  - Add a better UI error display, since this is proprietary by AIRS and for AIRS we can display the direct error message but make it more user-friendly

## 3. Dashboard Tab
Collect and display metrics including:
- Token usage
- Cost (in USD) collected since first usage
- General usage stats
- Worker time spent
- API call failures (both cloud and local)
- Logs
- And more

## 4. Graphify Integration
- Recreate the Graphify tool (it's small in scope) so that AutoDev agent workers can invoke it. This can be used as a **graph-based tool for codebase dissection**.
- The same tool itself needs to be able to make a markdown similar to the one i make on `autodev/docs/codebase/` that can list all the functions inside a single file and the description of each functions and the variable. We can do this by listing all the functions inside a file similar to how IDE does it, then list all the inputs, variables, and get the Dostring if exists. Essentially trying to create a comprehensive documentation of every file in a codebase without any LLM or AI assistance.
- Make a new tab for Graphify visualization that will run visualization on the codebase and display it as a graph itself with color coded connection

## 5. Feature Comparison — AutoDev vs. Windsurf
List current AutoDev features, compare them against Windsurf, and identify what can be implemented.