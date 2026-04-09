
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
1. Update pricing constants with actual values
2. Update the chat down right area that we can select model from there
3. Add thinking models and support with reasoning effort tunable from the settings
4. Add temperature and top-p parameters and each model can have an area we can select the values and saved properly in a configuration file similar to `model_registry.py`
5. Create a proper Ollama setup with local LLM providers using Ollama
6. Implement prompt caching for all LLM providers in Cloud mode
7. Add more models (o1, o3, gemini-2.0-flash, etc.)
8. Add model-specific context window validation

## 3. Dashboard Tab
Collect and display metrics including:
- Token usage
- General usage stats
- Worker time spent
- API call failures (both cloud and local)
- Logs
- And more

## 4. Graphify Integration
Recreate the Graphify tool (it's small in scope) so that AutoDev agent workers can invoke it. This can be used as a **graph-based tool for codebase dissection**.

## 5. Feature Comparison — AutoDev vs. Windsurf
List current AutoDev features, compare them against Windsurf, and identify what can be implemented.