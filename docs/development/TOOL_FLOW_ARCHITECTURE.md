# Tool Flow Architecture: Tauri → Core Orchestrator → LivingAgent

> **Document**: Detailed explanation of how AutoDev tools flow from Tauri to worker agents and execute  
> **Audience**: Developers implementing tool execution, bridge handlers, and agent lifecycle  
> **Related**: `autodev-master-plan.md` Section 4.2 "Tool Flow: Tauri → Core Orchestrator → LivingAgent → Execution"

---

## Overview

When a worker recipe triggers in AutoDev, tools flow through **4 distinct layers**:

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: Tauri (Rust) — Tool Definition                    │
│ Defines AutoDev tools (read_file, write_file, bash, git)   │
└────────────────────┬────────────────────────────────────────┘
                     │ HTTP POST /api/v1/agent/invoke
                     │ { "tools": [...] }
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ Layer 2: Core Orchestrator (Python) — Tool Registration    │
│ Receives AutoDev tools + discovers Internal + MCP tools    │
└────────────────────┬────────────────────────────────────────┘
                     │ Combines all tools
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ Layer 3: LivingAgent — Tool Provision to LLM               │
│ Registers all tools with litellm provider                   │
└────────────────────┬────────────────────────────────────────┘
                     │ LLM sees tools, calls one
                     ▼
┌─────────────────────────────────────────────────────────────┐
│ Layer 4: Execution — Tool Execution & Result Handling      │
│ UnifiedToolHandler routes to correct executor               │
└─────────────────────────────────────────────────────────────┘
```

---

## Layer 1: Tauri Tool Definition

### Where Tools Are Defined

```
src-tauri/src/agent/builtin_tools/
├── mod.rs
├── read_file.rs
├── write_file.rs
├── edit_file.rs
├── bash.rs
├── list_directory.rs
├── grep.rs
├── glob.rs
└── git/
    ├── status.rs
    ├── commit.rs
    ├── branch.rs
    └── diff.rs
```

### Tool Definition Structure

Each tool is a Rust struct implementing a `Tool` trait:

```rust
// src-tauri/src/agent/builtin_tools/read_file.rs

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    
    fn description(&self) -> &str {
        "Read file contents at specified path"
    }
    
    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute file path"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line offset (optional)"
                }
            },
            "required": ["path"]
        })
    }
    
    async fn execute(&self, input: serde_json::Value) -> Result<ToolResult> {
        let path = input["path"].as_str().ok_or("path required")?;
        let contents = std::fs::read_to_string(path)?;
        Ok(ToolResult {
            success: true,
            output: contents
        })
    }
}
```

### Tool Registration in PA Mode

In PA mode, tools are registered directly with the LLM:

```rust
// src-tauri/src/agent/engine.rs

impl AgentEngine {
    async fn execute_pa_mode(&self, request: AgentRequest) -> Result<()> {
        // 1. Create tool registry
        let mut tools = vec![
            Box::new(ReadFileTool) as Box<dyn Tool>,
            Box::new(WriteFileTool),
            Box::new(BashTool),
            Box::new(GitStatusTool),
            // ... more tools
        ];
        
        // 2. Pass directly to LLM
        let response = self.llm_provider.stream(
            &request.messages,
            &tools  // ← Tools go directly to LLM
        ).await?;
        
        // 3. Execute tools locally
        for tool_call in response.tool_calls {
            let result = self.execute_tool_locally(&tool_call).await?;
            // ... append result to conversation
        }
    }
}
```

---

## Layer 2: Core Orchestrator Tool Registration

### HTTP Request from Tauri

When Tauri sends a worker recipe request:

```http
POST http://localhost:4000/api/v1/agent/invoke
Content-Type: application/json

{
    "session_id": "sess_abc123",
    "recipe_id": "worker-backend-poll",
    "agent_role": "worker-backend",
    "tools": [
        {
            "name": "read_file",
            "description": "Read file contents",
            "inputSchema": { ... }
        },
        {
            "name": "write_file",
            "description": "Write file contents",
            "inputSchema": { ... }
        },
        {
            "name": "bash",
            "description": "Execute shell command",
            "inputSchema": { ... }
        },
        {
            "name": "git_status",
            "description": "Get git status",
            "inputSchema": { ... }
        }
    ]
}
```

### Tool Discovery in Core Orchestrator

```python
# src/airs/agent/living_agent.py

class LivingAgent:
    async def initialize_from_request(self, request: AgentRequest):
        """Initialize agent with tools from request + internal + MCP"""
        
        # 1. Get AutoDev tools from HTTP request
        autodev_tools = request.tools  # Passed from Tauri
        print(f"Received {len(autodev_tools)} AutoDev tools from Tauri")
        
        # 2. Get internal tools from agent config
        agent_config = self.config  # e.g., {"internal_tools": ["web_search"]}
        internal_tool_ids = agent_config.get("internal_tools", [])
        
        # Import the singleton bridge (created at module import time)
        from src.internal_tools import internal_bridge
        internal_tools = internal_bridge.get_tools(internal_tool_ids)
        print(f"Loaded {len(internal_tools)} internal tools: {internal_tool_ids}")
        
        # 3. Get MCP tools from MCP bridge
        mcp_tools = await self.mcp_bridge.get_tools()
        print(f"Discovered {len(mcp_tools)} MCP tools")
        
        # 4. Combine all tools
        all_tools = autodev_tools + internal_tools + mcp_tools
        print(f"Total tools available: {len(all_tools)}")
        
        # 5. Store for later use
        self.all_tools = all_tools
        self.autodev_tools = autodev_tools
        self.internal_tools = internal_tools
        self.mcp_tools = mcp_tools
```

### Tool Registration with LLM

```python
# src/airs/agent/living_agent.py

class LivingAgent:
    async def initialize_llm_tools(self):
        """Register all tools with the LLM provider"""
        
        # Convert all tools to ToolSpec format
        tool_specs = []
        for tool in self.all_tools:
            tool_specs.append({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.input_schema
            })
        
        # Register with litellm provider
        self.llm_provider.add_tools(tool_specs)
        print(f"Registered {len(tool_specs)} tools with LLM")
```

### Handler Registration with UnifiedToolHandler

```python
# src/airs/agent/living_agent.py

class LivingAgent:
    async def initialize_bridges(self):
        """Register tool handlers with UnifiedToolHandler"""
        
        from src.airs.bridges.unified_tool_handler import UnifiedToolHandler
        
        handler = UnifiedToolHandler()
        
        # 1. Register internal tool handler
        handler.register_internal_handler(
            handler_func=self.internal_bridge.handle_tool_call,
            tools=[t.name for t in self.internal_tools]
        )
        
        # 2. Register MCP tool handler
        handler.register_mcp_handler(
            handler_func=self.mcp_bridge.handle_tool_call,
            tools=[t.name for t in self.mcp_tools]
        )
        
        # 3. Register AutoDev tool handler (NEW)
        handler.register_autodev_handler(
            handler_func=self.autodev_handler.handle_tool_call,
            tools=[t.name for t in self.autodev_tools]
        )
        
        self.unified_handler = handler
```

---

## Layer 3: LivingAgent Tool Provision

### LLM Receives All Tools

```python
# src/airs/agent/living_agent.py

async def process_message(self, user_message: str):
    """Main agent loop"""
    
    # 1. Add user message
    self.conversation.append({
        "role": "user",
        "content": user_message
    })
    
    # 2. Call LLM with ALL tools (internal + MCP + AutoDev)
    response = await self.llm_provider.complete(
        messages=self.conversation,
        tools=self.all_tools  # ← All tools go to LLM
    )
    
    # 3. Stream response to UI
    async for chunk in response.stream():
        await self.sse_broker.emit("agent:stream-token", chunk)
    
    # 4. If LLM called a tool, execute it
    if response.tool_calls:
        for tool_call in response.tool_calls:
            result = await self.tool_executor.execute(tool_call)
            self.conversation.append({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": tool_call.id,
                    "content": result
                }]
            })
```

### LLM Tool Call Example

The LLM sees all tools and can call any of them:

```json
{
    "type": "tool_use",
    "id": "call_123",
    "name": "read_file",
    "arguments": {
        "path": "/src/main.py"
    }
}
```

Or:

```json
{
    "type": "tool_use",
    "id": "call_124",
    "name": "web_search__internal",
    "arguments": {
        "query": "latest Python best practices"
    }
}
```

Or:

```json
{
    "type": "tool_use",
    "id": "call_125",
    "name": "github_create_issue",
    "arguments": {
        "title": "Add health endpoint",
        "body": "..."
    }
}
```

---

## Layer 4: Tool Execution

### UnifiedToolHandler Routes Tool Calls

```python
# src/airs/bridges/unified_tool_handler.py

class UnifiedToolHandler:
    async def handle_tool_call(self, tool_call: ToolCall) -> Dict[str, Any]:
        """Route tool call to appropriate handler"""
        
        tool_name = tool_call.name
        
        # Route based on tool name suffix/prefix pattern
        if tool_name.endswith("__internal"):
            # Internal tool (e.g., "web_search__internal")
            return await self.internal_handler(tool_call)
        
        elif tool_name.endswith("__autodev"):
            # AutoDev tool (e.g., "read_file__autodev", "write_file__autodev", "bash__autodev")
            return await self.autodev_handler(tool_call)
        
        elif tool_name.startswith("github_") or tool_name.startswith("slack_"):
            # MCP tool (e.g., "github_create_issue", "slack_send_message")
            return await self.mcp_handler(tool_call)
        
        else:
            # Unknown tool
            return {"error": f"Unknown tool: {tool_name}"}
```

### MemoryAwareToolExecutor Tracks Execution

```python
# src/airs/execution/tool_executor.py

class MemoryAwareToolExecutor:
    async def execute(self, tool_call: ToolCall) -> Dict[str, Any]:
        """Execute tool with memory awareness"""
        
        # 1. Create FSM for this execution
        fsm = ToolExecutionFSM(
            tool_name=tool_call.name,
            session_id=self.session_id
        )
        
        # 2. Register in global registry (for observability)
        registry = ToolExecutionRegistry()
        registry.register(fsm, ToolExecutionInfo(
            call_id=tool_call.id,
            tool_name=tool_call.name,
            session_id=self.session_id
        ))
        
        # 3. Check memory: has this tool failed before?
        past_failures = await self.memory.retrieve(
            f"tool_failures:{tool_call.name}"
        )
        if past_failures:
            logger.warning(f"Tool {tool_call.name} failed before: {past_failures}")
        
        # 4. Execute via UnifiedToolHandler
        result = await self.unified_handler.handle_tool_call(tool_call)
        
        # 5. Store outcome in memory
        await self.memory.store(
            f"tool_execution:{tool_call.name}",
            {
                "success": result.get("error") is None,
                "result": result,
                "timestamp": datetime.now().isoformat()
            }
        )
        
        # 6. Update FSM state
        fsm.mark_complete()
        
        return result
```

### AutoDev Tool Handler (NEW)

```python
# src/airs/bridges/autodev_handler.py (NEW FILE)

class AutoDevToolHandler:
    """Handles AutoDev tools (read_file, write_file, bash, git, etc.)"""
    
    async def handle_tool_call(self, tool_call: ToolCall) -> Dict[str, Any]:
        """Execute AutoDev tool"""
        
        tool_name = tool_call.name
        args = tool_call.arguments
        
        # Route to appropriate executor
        if tool_name == "read_file":
            return await self.execute_read_file(args)
        elif tool_name == "write_file":
            return await self.execute_write_file(args)
        elif tool_name == "bash":
            return await self.execute_bash(args)
        elif tool_name.startswith("git_"):
            return await self.execute_git(tool_name, args)
        else:
            return {"error": f"Unknown AutoDev tool: {tool_name}"}
    
    async def execute_read_file(self, args: Dict[str, Any]) -> Dict[str, Any]:
        """Execute read_file tool"""
        try:
            path = args["path"]
            offset = args.get("offset")
            limit = args.get("limit")
            
            with open(path, 'r') as f:
                contents = f.read()
            
            return {
                "success": True,
                "contents": contents,
                "path": path
            }
        except Exception as e:
            return {"error": str(e)}
    
    # ... more tool implementations
```

---

## Complete Tool Flow Example

### Scenario: Worker Backend Reads a File

```
1. Recipe triggers: worker-backend-poll.yaml
   ↓
2. Tauri sends POST /api/v1/agent/invoke with:
   - tools: [read_file__autodev, write_file__autodev, bash__autodev, git_status__autodev, ...]
   ↓
3. Core Orchestrator LivingAgent.__init__():
   - Receives AutoDev tools from request
   - Loads internal tools: [web_search__internal]
   - Discovers MCP tools: [github_create_issue, slack_send_message]
   - Combines: all_tools = [read_file__autodev, write_file__autodev, bash__autodev, git_status__autodev,
                            web_search__internal,
                            github_create_issue, slack_send_message]
   ↓
4. LLM provider.add_tools(all_tools)
   ↓
5. LLM processes message and decides to call read_file:
   {
       "type": "tool_use",
       "name": "read_file__autodev",
       "arguments": {"path": "/src/main.py"}
   }
   ↓
6. MemoryAwareToolExecutor.execute():
   - Create ToolExecutionFSM
   - Register in ToolExecutionRegistry
   - Check memory for past failures
   ↓
7. UnifiedToolHandler.handle_tool_call():
   - Tool name = "read_file__autodev" (ends with __autodev)
   - Route to autodev_handler
   ↓
8. AutoDevToolHandler.execute_read_file():
   - Strip __autodev suffix → "read_file"
   - Read file from filesystem
   - Return contents
   ↓
9. Store result in memory
   ↓
10. Append tool result to conversation
    ↓
11. Continue LLM loop
```

---

## Key Design Patterns

### 1. Tool Name Routing

| Pattern | Source | Handler | Example |
|---------|--------|---------|---------|
| `{name}__internal` | Internal tools | `internal_handler` | `web_search__internal` |
| `{name}__autodev` | AutoDev tools | `autodev_handler` | `read_file__autodev`, `write_file__autodev` |
| `{prefix}_{name}` | MCP tools | `mcp_handler` | `github_create_issue`, `slack_send_message` |

### 2. Tool Registration Flow

```
Tool Definition (Tauri)
    ↓
Tool Discovery (Core Orchestrator)
    ↓
Tool Registration (LivingAgent)
    ↓
Tool Provision (LLM)
    ↓
Tool Execution (UnifiedToolHandler)
    ↓
Result Storage (Memory + Conversation)
```

### 3. Memory-Aware Execution

Every tool execution is tracked:
- **Before**: Check if this tool failed before
- **During**: Execute with FSM tracking
- **After**: Store outcome in semantic memory

This enables agents to learn from every tool call automatically.

---

## Implementation Checklist

- [x] AutoDev tools defined in Rust (src-tauri/src/agent/builtin_tools/)
- [x] Tools passed via HTTP POST to core-orchestrator
- [x] LivingAgent receives and combines tools
- [x] UnifiedToolHandler routes by name pattern
- [ ] AutoDevToolHandler implementation (NEW)
- [ ] Tool execution tracking in ToolExecutionRegistry
- [ ] Memory storage of tool outcomes
- [ ] Error handling and retry logic
- [ ] Tool result streaming to UI via SSE

---

## Related Files

- `autodev-master-plan.md` — Section 4.2: Tool Flow architecture
- `DEVELOPMENT_PLAN.md` — Section 4.1: Dual-Mode Architecture
- `ARCHITECTURE_CLARIFICATIONS.md` — Dual-mode explanation
- `src-tauri/src/agent/builtin_tools/` — Tool definitions
- `src/airs/bridges/unified_tool_handler.py` — Tool routing
- `src/airs/execution/tool_executor.py` — Tool execution tracking
- `src/internal_tools/` — Internal tool system
