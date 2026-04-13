# Post-Phase 6 Features & Refinements

**THIS SHOULD BE DONE AFTER PHASE 1 - 6 IS FINISHED**

> **Note**: Most features from the original list have been moved into DEVELOPMENT_PLAN.md phases:
> - PA Memory → **Phase 4**
> - Model enhancements (thinking, temperature, context window) → **Phase 2**
> - Dashboard → **Phase 5**
> - Graphify → **Phase 6**

---

## Documentation Tasks (Do First)

### Developer Guide
Create a `developer-guide.md` file that documents the project structure, first time installation steps, key components, and how to contribute.

---

## Post-Phase 6: True Extras

These are features that don't fit into the core 6-phase roadmap:

### Bug Fixes & Provider-Specific Issues
- **Google Gemini Path Handling**: Investigate and fix Gemini's failure on reading files outside agent's working directory
  - Test with gemini-2.0-flash-lite
  - Refine agent behavior for Gemini compatibility

### Ongoing Maintenance (Not Phase-Specific)
- Add more models as they become available (o1, o3, gemini-2.0-flash, etc.)
- Error handling improvements (ongoing as system matures)
- UI polish based on user feedback

### Research & Analysis (Documentation Only)
- **Feature Comparison — AutoDev vs. Windsurf**: Analyze competitive landscape, identify gaps

### Future Experimental Ideas
- Voice input for chat (speech-to-text)
- Custom themes and deep UI customization
- Plugin marketplace for community extensions
- A/B testing framework for prompt variants
- Mobile companion app (view-only)
- Browser extension for context capture

---

## Summary: What Went Where

| Original Item | New Location |
|---------------|--------------|
| PA Memory System | **Phase 4** — Agent Memory + Subagents + Model Router |
| Thinking models + reasoning effort | **Phase 2** — Provider Support & Model Enhancements |
| Temperature + top-p parameters | **Phase 2** — Provider Support & Model Enhancements |
| Model context window validation | **Phase 2** — Provider Support & Model Enhancements |
| Prompt caching | **Phase 4** — Providers (cost optimization) |
| Ollama setup | **Phase 3** — Ollama provider (local models) |
| Dashboard Tab | **Phase 5** — Cost Dashboard + Analytics |
| Graphify Integration | **Phase 6** — Advanced Developer Tools |
| Developer guide | **Post-Phase 6** (documentation task) |
| Gemini path bug | **Post-Phase 6** (provider-specific fix) |
| Feature comparison | **Post-Phase 6** (documentation/analysis) |