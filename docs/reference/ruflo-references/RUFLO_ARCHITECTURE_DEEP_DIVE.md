# Ruflo Architecture Deep Dive

**Reference document for AutoDev inspiration from Ruflo's advanced architectural patterns**

This document provides an in-depth exploration of four key Ruflo concepts that could inspire improvements to AutoDev:

1. **SONA (Self-Optimizing Neural Architecture)** - Self-learning pattern optimization with sub-millisecond adaptation
2. **Swarm Topologies + Consensus Algorithms** - Hierarchical, mesh, ring coordination with Raft/BFT/Gossip
3. **Federation System** - Zero-trust cross-machine agent federation with budget circuit breakers
4. **Plugin System** - Extensible npm package architecture for modular capabilities

---

## 1. SONA (Self-Optimizing Neural Architecture)

### Overview

SONA is a runtime-adaptive learning system that optimizes agent behavior through trajectory-based learning, LoRA (Low-Rank Adaptation), and EWC (Elastic Weight Consolidation) for continual learning without catastrophic forgetting.

### Key Features

- **Performance Targets**: Adaptation <0.05ms, Pattern retrieval <1ms, Learning step <10ms
- **Five Learning Modes**: real-time, balanced, research, edge, batch
- **LoRA-based Adaptation**: Rank 1-16 for efficient model fine-tuning
- **EWC Continual Learning**: Prevents catastrophic forgetting across tasks
- **Trajectory-based Learning**: Learns from task execution sequences
- **Pattern Clustering**: Groups similar patterns for efficient retrieval
- **Persistence**: Serialize/deserialize state for process restarts

### Code Locations

**Core Implementation:**
- `v3/@claude-flow/neural/src/sona-manager.ts` (907 lines) - Main SONAManager class
- `v3/@claude-flow/neural/src/sona-integration.ts` - Integration bridge
- `v3/@claude-flow/neural/docs/SONA_INTEGRATION.md` (461 lines) - Integration guide

**Mode Implementations:**
- `v3/@claude-flow/neural/src/modes/real-time.js` - Sub-millisecond adaptation mode
- `v3/@claude-flow/neural/src/modes/balanced.js` - General purpose mode
- `v3/@claude-flow/neural/src/modes/research.js` - Deep exploration mode
- `v3/@claude-flow/neural/src/modes/edge.js` - Resource-constrained mode
- `v3/@claude-flow/neural/src/modes/batch.js` - High-throughput mode

**Type Definitions:**
- `v3/@claude-flow/neural/src/types.ts` - SONA types (SONAMode, SONAModeConfig, Trajectory, Pattern, etc.)

**MCP Tools:**
- `v3/mcp/tools/sona-tools.ts` - MCP tool definitions for SONA
- `v3/@claude-flow/cli/src/mcp-tools/neural-tools.ts` - Neural tool bridge

### Architecture Details

#### Learning Modes Configuration

```typescript
const MODE_CONFIGS: Record<SONAMode, SONAModeConfig> = {
  'real-time': {
    mode: 'real-time',
    loraRank: 2,
    learningRate: 0.001,
    batchSize: 32,
    trajectoryCapacity: 1000,
    patternClusters: 25,
    qualityThreshold: 0.7,
    maxLatencyMs: 0.5,
    memoryBudgetMb: 25,
    ewcLambda: 2000,
  },
  'balanced': {
    mode: 'balanced',
    loraRank: 4,
    learningRate: 0.002,
    batchSize: 32,
    trajectoryCapacity: 3000,
    patternClusters: 50,
    qualityThreshold: 0.5,
    maxLatencyMs: 18,
    memoryBudgetMb: 50,
    ewcLambda: 2000,
  },
  'research': {
    mode: 'research',
    loraRank: 16,
    learningRate: 0.002,
    batchSize: 64,
    trajectoryCapacity: 10000,
    patternClusters: 100,
    qualityThreshold: 0.2,
    maxLatencyMs: 100,
    memoryBudgetMb: 100,
    ewcLambda: 2500,
  },
  'edge': {
    mode: 'edge',
    loraRank: 1,
    learningRate: 0.001,
    batchSize: 16,
    trajectoryCapacity: 200,
    patternClusters: 15,
    qualityThreshold: 0.8,
    maxLatencyMs: 1,
    memoryBudgetMb: 5,
    ewcLambda: 1500,
  },
  'batch': {
    mode: 'batch',
    loraRank: 8,
    learningRate: 0.002,
    batchSize: 128,
    trajectoryCapacity: 5000,
    patternClusters: 75,
    qualityThreshold: 0.4,
    maxLatencyMs: 50,
    memoryBudgetMb: 75,
    ewcLambda: 2000,
  },
};
```

#### Mode-Specific Optimizations

```typescript
const MODE_OPTIMIZATIONS: Record<SONAMode, ModeOptimizations> = {
  'real-time': {
    enableSIMD: true,
    useMicroLoRA: true,
    gradientCheckpointing: false,
    useHalfPrecision: true,
    patternCaching: true,
    asyncUpdates: true,
  },
  'balanced': {
    enableSIMD: true,
    useMicroLoRA: false,
    gradientCheckpointing: false,
    useHalfPrecision: false,
    patternCaching: true,
    asyncUpdates: false,
  },
  'research': {
    enableSIMD: true,
    useMicroLoRA: false,
    gradientCheckpointing: true,
    useHalfPrecision: false,
    patternCaching: true,
    asyncUpdates: false,
  },
  'edge': {
    enableSIMD: false,
    useMicroLoRA: true,
    gradientCheckpointing: false,
    useHalfPrecision: true,
    patternCaching: false,
    asyncUpdates: true,
  },
  'batch': {
    enableSIMD: true,
    useMicroLoRA: false,
    gradientCheckpointing: true,
    useHalfPrecision: true,
    patternCaching: true,
    asyncUpdates: true,
  },
};
```

#### Trajectory Management

SONA learns from task execution trajectories:

```typescript
interface Trajectory {
  trajectoryId: string;
  context: string;
  domain: 'general' | 'code' | 'creative' | 'reasoning' | 'chat' | 'math';
  steps: TrajectoryStep[];
  qualityScore: number;
  isComplete: boolean;
  startTime: number;
  endTime?: number;
}

interface TrajectoryStep {
  stepId: string;
  timestamp: number;
  action: string;
  stateBefore: Float32Array;
  stateAfter: Float32Array;
  reward: number;
  metadata?: Record<string, unknown>;
}
```

#### LoRA Weight Management

LoRA (Low-Rank Adaptation) enables efficient model fine-tuning:

```typescript
interface LoRAWeights {
  adapterId: string;
  A: Map<string, Float32Array>;  // (hidden_dim, rank) matrices
  B: Map<string, Float32Array>;  // (rank, hidden_dim) matrices
  createdAt: number;
  updatedAt: number;
  iterations: number;
  domain: string;
}

interface LoRAConfig {
  rank: number;
  alpha: number;
  dropout: number;
  targetModules: string[];
  microLoRA: boolean;
}
```

#### EWC (Elastic Weight Consolidation)

Prevents catastrophic forgetting during continual learning:

```typescript
interface EWCConfig {
  lambda: number;
  decay: number;
  fisherSamples: number;
  minFisher: number;
  online: boolean;
}

interface EWCState {
  means: Map<string, Float32Array>;
  fisher: Map<string, Float32Array>;
  taskCount: number;
  lastConsolidation: number;
}
```

#### Pattern Matching

Finds similar patterns using vector similarity:

```typescript
async findSimilarPatterns(
  embedding: Float32Array,
  k: number = 3
): Promise<PatternMatch[]>

interface Pattern {
  patternId: string;
  name: string;
  strategy: string;
  embedding: Float32Array;
  qualityHistory: number[];
  successRate: number;
  usageCount: number;
  evolutionHistory: string[];
  createdAt: number;
  updatedAt: number;
}

interface PatternMatch {
  pattern: Pattern;
  similarity: number;
  latencyMs: number;
}
```

#### Persistence

SONA supports serialization for process restarts:

```typescript
serialize(): unknown {
  return deepEncode({
    schemaVersion: 1,
    currentMode: this.currentMode,
    config: this.config,
    optimizations: this.optimizations,
    trajectories: this.trajectories,
    patterns: this.patterns,
    loraWeights: this.loraWeights,
    ewcState: this.ewcState,
    stats: this.stats,
    isInitialized: this.isInitialized,
    operationCount: this.operationCount,
    totalLatencyMs: this.totalLatencyMs,
    learningCycles: this.learningCycles,
    lastStatsUpdate: this.lastStatsUpdate,
  });
}

deserialize(state: unknown): void {
  // Restore all state from serialized snapshot
  // Mode implementation is rebuilt for the saved mode
  // Event listeners are NOT restored (re-register manually)
}
```

### Integration Example

```typescript
import {
  createSONALearningEngine,
  type Trajectory,
  type Context,
} from '@claude-flow/neural';
import { getModeConfig } from '@claude-flow/neural';

// Create SONA engine with balanced mode
const modeConfig = getModeConfig('balanced');
const sona = createSONALearningEngine('balanced', modeConfig);

// Learn from a trajectory
const trajectory: Trajectory = {
  trajectoryId: 'traj-001',
  context: 'Implement authentication',
  domain: 'code',
  steps: [
    {
      stepId: 'step-1',
      timestamp: Date.now(),
      action: 'analyze requirements',
      stateBefore: new Float32Array(768).fill(0.1),
      stateAfter: new Float32Array(768).fill(0.2),
      reward: 0.8,
    },
  ],
  qualityScore: 0.88,
  isComplete: true,
  startTime: Date.now(),
};

await sona.learn(trajectory);
console.log(`Learning time: ${sona.getLearningTime()}ms`);

// Adapt to new context
const context: Context = {
  domain: 'code',
  queryEmbedding: new Float32Array(768).fill(0.15),
};

const adapted = await sona.adapt(context);
console.log(`Confidence: ${adapted.confidence}`);
console.log(`Suggested route: ${adapted.suggestedRoute}`);
```

### Performance Characteristics

| Mode       | Avg Time | Target  | Memory  |
|------------|----------|---------|---------|
| Real-time  | ~0.03ms  | <0.05ms | 100MB   |
| Balanced   | ~0.04ms  | <0.05ms | 200MB   |
| Research   | ~0.08ms  | <0.10ms | 500MB   |
| Edge       | ~0.02ms  | <0.05ms | 50MB    |
| Batch      | ~0.05ms  | <0.10ms | 1GB     |

### AutoDev Inspiration Opportunities

1. **Task Assignment Optimization**: Use SONA to learn which worker types perform best for specific task types
2. **Recipe Selection**: Learn which recipes work best for different contexts
3. **Model Routing**: Optimize LLM provider selection based on task characteristics
4. **Failure Prediction**: Predict task failures based on trajectory patterns
5. **Adaptive Timeout**: Dynamically adjust task timeouts based on learned performance

---

## 2. Swarm Topologies + Consensus Algorithms

### Overview

Ruflo's swarm coordination system provides multiple topology options (hierarchical, mesh, ring, star, adaptive, hierarchical-mesh) with pluggable consensus algorithms (Raft, Byzantine Fault Tolerance, Gossip, CRDT, Quorum) for fault-tolerant multi-agent coordination.

### Key Features

- **Six Topology Types**: hierarchical, mesh, ring, star, adaptive, hierarchical-mesh
- **Five Consensus Algorithms**: Raft, BFT, Gossip, CRDT, Quorum
- **Anti-Drift Mechanisms**: Specialized roles, agent count limits, coordinator enforcement
- **Worktree Isolation**: Each agent works in its own git worktree
- **Monitor Streaming**: Real-time swarm status via SSE
- **Hive-Mind Consensus**: Queen-led hierarchy with collective memory
- **15-Agent Architecture**: Scalable swarm with phase-based execution

### Code Locations

**Core Coordination:**
- `v3/@claude-flow/swarm/src/coordination/swarm-hub.ts` (777 lines) - Compatibility layer over UnifiedSwarmCoordinator
- `v3/@claude-flow/swarm/src/unified-coordinator/` - Canonical coordination engine
- `v3/@claude-flow/swarm/src/application/services/swarm-application-service.ts` - Application service

**Type Definitions:**
- `v3/@claude-flow/shared/src/types/swarm.types.ts` (256 lines) - Swarm type definitions
- `v3/@claude-flow/swarm/src/shared/types/` - Internal type definitions

**Plugin Implementation:**
- `plugins/ruflo-swarm/README.md` (87 lines) - Plugin documentation
- `plugins/ruflo-swarm/agents/coordinator.md` (32 lines) - Coordinator agent definition
- `plugins/ruflo-swarm/agents/architect.md` - Architect agent definition

**CLI Integration:**
- `v3/@claude-flow/cli/src/commands/swarm.ts` - CLI swarm commands
- `v3/@claude-flow/cli/src/mcp-tools/swarm-tools.ts` - MCP tool definitions

**Consensus Tests:**
- `v3/@claude-flow/swarm/__tests__/consensus.test.ts` - Consensus algorithm tests

### Architecture Details

#### Topology Types

```typescript
export type SwarmTopology = 
  | 'hierarchical'
  | 'mesh'
  | 'ring'
  | 'star'
  | 'adaptive'
  | 'hierarchical-mesh';

export const TopologyPresets: Record<SwarmTopology, Partial<ISwarmConfig>> = {
  hierarchical: {
    topology: 'hierarchical',
    coordination: {
      consensusRequired: false,
      timeoutMs: 5000,
      retryPolicy: { maxRetries: 3, backoffMs: 1000 }
    }
  },
  mesh: {
    topology: 'mesh',
    coordination: {
      consensusRequired: true,
      timeoutMs: 10000,
      retryPolicy: { maxRetries: 5, backoffMs: 500 }
    }
  },
  ring: {
    topology: 'ring',
    coordination: {
      consensusRequired: true,
      timeoutMs: 8000,
      retryPolicy: { maxRetries: 4, backoffMs: 750 }
    }
  },
  star: {
    topology: 'star',
    coordination: {
      consensusRequired: false,
      timeoutMs: 3000,
      retryPolicy: { maxRetries: 2, backoffMs: 500 }
    }
  },
  adaptive: {
    topology: 'adaptive',
    autoScale: {
      enabled: true,
      minAgents: 1,
      maxAgents: 20,
      scaleUpThreshold: 0.8,
      scaleDownThreshold: 0.3
    },
    coordination: {
      consensusRequired: true,
      timeoutMs: 10000,
      retryPolicy: { maxRetries: 5, backoffMs: 500 }
    }
  },
  'hierarchical-mesh': {
    topology: 'hierarchical-mesh',
    coordination: {
      consensusRequired: true,
      timeoutMs: 8000,
      retryPolicy: { maxRetries: 4, backoffMs: 750 }
    }
  }
};
```

#### Consensus Algorithms

```typescript
export type ConsensusAlgorithm = 
  | 'raft'
  | 'bft'  // Byzantine Fault Tolerance
  | 'gossip'
  | 'crdt'
  | 'quorum';

interface ConsensusConfig {
  algorithm: ConsensusAlgorithm;
  threshold: number;  // 0.66 for 2/3 majority
  timeoutMs: number;
  maxRounds: number;
  requireQuorum: boolean;
}
```

#### Swarm Configuration

```typescript
interface SwarmConfig {
  topology: {
    type: SwarmTopology;
    maxAgents: number;
    replicationFactor: number;
    partitionStrategy: 'hash' | 'random' | 'consistent';
    failoverEnabled: boolean;
    autoRebalance: boolean;
  };
  consensus: ConsensusConfig;
  messageBus: {
    maxQueueSize: number;
    processingIntervalMs: number;
    ackTimeoutMs: number;
    retryAttempts: number;
    enablePersistence: boolean;
    compressionEnabled: boolean;
  };
  maxAgents: number;
  maxTasks: number;
  heartbeatIntervalMs: number;
  healthCheckIntervalMs: number;
  taskTimeoutMs: number;
  autoScaling: boolean;
  autoRecovery: boolean;
}
```

#### Anti-Drift Defaults

For coding swarms, the canonical defaults that prevent agent drift:

| Setting | Value | Rationale |
|---------|-------|-----------|
| `topology` | `hierarchical` | Coordinator catches divergence |
| `maxAgents` | 6–8 | Smaller team = less drift |
| `strategy` | `specialized` | Clear roles, no overlap |
| `consensus` | `raft` | Leader maintains authoritative state |
| `memory` | `hybrid` | SQLite + AgentDB for both fast + durable |

For 10+ agent teams, use `hierarchical-mesh` (queen + peer communication).

#### Swarm Hub Interface

```typescript
export interface ISwarmHub {
  // Lifecycle
  initialize(config?: Partial<SwarmConfig>): Promise<void>;
  shutdown(): Promise<void>;
  isInitialized(): boolean;

  // Agent Management
  spawnAgent(agentId: AgentId): Promise<AgentState>;
  spawnAllAgents(): Promise<Map<AgentId, AgentState>>;
  spawnAgentsByDomain(domain: AgentDomain): Promise<AgentState[]>;
  terminateAgent(agentId: AgentId): Promise<boolean>;

  // Task Management
  submitTask(spec: TaskSpec): TaskDefinition;
  submitBatchTasks(specs: TaskSpec[]): TaskDefinition[];
  assignNextTask(agentId: AgentId): TaskDefinition | undefined;
  completeTask(taskId: TaskId, result: TaskResult): void;

  // Phase Management
  getCurrentPhase(): PhaseId;
  advancePhase(): PhaseId;
  getPhaseDefinition(phaseId: PhaseId): PhaseDefinition;

  // Milestone Tracking
  getMilestones(): MilestoneDefinition[];
  completeMilestone(milestoneId: string): void;

  // Messaging
  sendMessage<T>(message: Omit<SwarmMessage<T>, 'id' | 'timestamp'>): void;
  broadcast<T>(from: AgentId, type: MessageType, payload: T): void;
  onMessage<T>(handler: MessageHandler<T>): () => void;

  // Metrics & Status
  getState(): SwarmState;
  getMetrics(): SwarmMetrics;
  getAgentRegistry(): IAgentRegistry;
  getTaskOrchestrator(): ITaskOrchestrator;

  // Events
  onSwarmEvent(handler: EventHandler): () => void;
}
```

#### Consensus Request/Response

```typescript
interface ConsensusRequest<T = unknown> {
  topic: string;
  options: T[];
  requiredVotes: number | 'majority' | 'all';
  timeout: number;
  voters?: string[];
}

interface ConsensusResponse<T = unknown> {
  topic: string;
  decision: T | null;
  votes: Map<string, T>;
  consensus: boolean;
  votingDuration: number;
  participatingAgents: string[];
}
```

#### Distributed Locking

```typescript
interface DistributedLock {
  resourceId: string;
  holderId: string;
  acquiredAt: Date;
  expiresAt: Date;
  renewCount: number;
}

interface LockAcquisitionResult {
  acquired: boolean;
  lock?: DistributedLock;
  waitTime?: number;
  error?: Error;
}

interface DeadlockDetectionResult {
  detected: boolean;
  agents: string[];
  resources: string[];
  cycle?: string[][];
  suggestedResolution?: string;
}
```

#### Swarm Metrics

```typescript
interface SwarmMetrics {
  topology: SwarmTopology;
  status: CoordinationStatus;
  agentCount: number;
  activeAgentCount: number;
  taskCount: number;
  completedTasks: number;
  failedTasks: number;
  throughput: number;
  avgLatencyMs: number;
  p95LatencyMs: number;
  p99LatencyMs: number;
  successRate: number;
  resourceUtilization: number;
  messagesSent: number;
  messagesReceived: number;
  consensusRounds: number;
  locksAcquired: number;
  deadlocksDetected: number;
  uptime: number;
  timestamp: Date;
}
```

### Coordinator Agent

The coordinator agent manages swarm lifecycle:

```markdown
---
name: coordinator
description: Swarm coordinator that manages agent lifecycle, task assignment, and anti-drift enforcement
model: sonnet
---
You are the swarm coordinator within a Ruflo hierarchical topology. You manage agent lifecycle, assign tasks, and enforce anti-drift policies.

Responsibilities:
1. Initialize the swarm: `npx @claude-flow/cli@latest swarm init --topology hierarchical --max-agents 8 --strategy specialized`
2. Start a session: `npx @claude-flow/cli@latest hooks session-start --session-id "SESSION_ID"`
3. Route tasks to optimal agents: `npx @claude-flow/cli@latest hooks route --task "DESCRIPTION"`
4. Monitor progress and reassign stalled work.
5. End session with metrics: `npx @claude-flow/cli@latest hooks session-end --export-metrics true`

Anti-drift rules:
- Keep agent count at 6-8 for tight coordination.
- Use specialized strategy so roles do not overlap.
- Run `post-task` hooks after every task completion for learning.
- Store coordination decisions in memory namespace "swarm".
```

### MCP Tools

The ruflo-swarm plugin exposes 12 MCP tools:

| Family | Count | Tools |
|--------|------:|-------|
| `swarm_*` | 4 | `swarm_init`, `swarm_status`, `swarm_shutdown`, `swarm_health` |
| `agent_*` | 8 | `agent_spawn`, `agent_execute`, `agent_terminate`, `agent_status`, `agent_list`, `agent_pool`, `agent_health`, `agent_update` |

### AutoDev Inspiration Opportunities

1. **Multi-Topology Worker Coordination**: Beyond hierarchical Supervisor→Workers, add mesh topology for peer-to-peer worker collaboration
2. **Consensus-Based Decision Making**: Use consensus algorithms for QC approval, task acceptance, and conflict resolution
3. **Anti-Drift Mechanisms**: Implement specialized worker roles and agent count limits to prevent task drift
4. **Distributed Locking**: Add distributed locks for resource contention (e.g., git operations, file access)
5. **Phase-Based Execution**: Organize work into phases (foundation, core, integration, release) with milestone tracking
6. **Deadlock Detection**: Detect and resolve deadlocks in worker task dependencies
7. **Adaptive Scaling**: Dynamically scale worker count based on workload

---

## 3. Federation System (Cross-Machine Agent Federation)

### Overview

Ruflo's federation system enables zero-trust cross-machine agent collaboration with PII-gated data flow, 5-tier trust model, compliance-grade audit trails, and budget circuit breakers to prevent runaway delegation loops.

### Key Features

- **Zero-Trust Federation**: mTLS + ed25519 challenge-response handshake before any data moves
- **PII Pipeline**: 14-type detection with per-trust-level policies (BLOCK/REDACT/HASH/PASS)
- **5-Tier Trust Model**: UNTRUSTED → VERIFIED → ATTESTED → TRUSTED → PRIVILEGED with behavioral scoring
- **Compliance Modes**: HIPAA, SOC2, GDPR audit trails as first-class primitives
- **Secure Messaging**: HMAC-signed envelopes with dual AI Defence gates
- **Byzantine Consensus**: BFT for state mutations across untrusted federation peers
- **Budget Circuit Breaker (ADR-097)**: Per-call `maxHops`, `maxTokens`, `maxUsd` caps with constant-string errors
- **Automatic Downgrade**: Immediate trust downgrade on threat detection or HMAC failure

### Code Locations

**Core Federation:**
- `v3/@claude-flow/plugin-agent-federation/src/application/federation-coordinator.ts` (368 lines) - Main coordinator
- `v3/@claude-flow/plugin-agent-federation/src/domain/entities/federation-node.ts` - Federation node entity
- `v3/@claude-flow/plugin-agent-federation/src/domain/entities/federation-session.ts` - Session entity
- `v3/@claude-flow/plugin-agent-federation/src/domain/entities/federation-envelope.ts` - Message envelope
- `v3/@claude-flow/plugin-agent-federation/src/domain/entities/trust-level.ts` - Trust level definitions

**Services:**
- `v3/@claude-flow/plugin-agent-federation/src/domain/services/discovery-service.ts` - Peer discovery
- `v3/@claude-flow/plugin-agent-federation/src/domain/services/handshake-service.ts` - mTLS handshake
- `v3/@claude-flow/plugin-agent-federation/src/domain/services/routing-service.ts` - Message routing
- `v3/@claude-flow/plugin-agent-federation/src/domain/services/audit-service.ts` - Audit logging
- `v3/@claude-flow/plugin-agent-federation/src/domain/services/pii-pipeline-service.ts` - PII detection/policy
- `v3/@claude-flow/plugin-agent-federation/src/application/trust-evaluator.ts` - Trust scoring
- `v3/@claude-flow/plugin-agent-federation/src/application/policy-engine.ts` - Policy enforcement

**Budget Enforcement:**
- `v3/@claude-flow/plugin-agent-federation/src/domain/value-objects/federation-budget.ts` - Budget validation/enforcement

**Plugin Implementation:**
- `plugins/ruflo-federation/README.md` (98 lines) - Plugin documentation
- `plugins/ruflo-federation/agents/federation-coordinator.md` (57 lines) - Coordinator agent
- `plugins/ruflo-federation/docs/adrs/0001-federation-contract.md` - ADR documentation

**MCP Tools:**
- `v3/mcp/tools/federation-tools.ts` - MCP tool definitions
- `v3/@claude-flow/swarm/src/federation-hub.ts` - Federation hub integration

**Claims Integration:**
- `v3/@claude-flow/claims/src/infrastructure/federation-bridge.ts` - Claims system bridge

### Architecture Details

#### Trust Levels

```typescript
enum TrustLevel {
  UNTRUSTED = 0,    // Discovery only
  VERIFIED = 1,     // Status, ping
  ATTESTED = 2,     // Send/receive tasks, query memory (redacted)
  TRUSTED = 3,      // Share context, collaborative execution
  PRIVILEGED = 4,   // Full memory, remote agent spawning
}
```

#### Trust Scoring Formula

```
trust_score = 0.4 × success_rate + 0.2 × uptime + 0.2 × (1 - threat_penalty) + 0.2 × data_integrity
```

#### Budget Circuit Breaker (ADR-097 Phase 1)

```typescript
export interface SendOptions {
  readonly budget?: { maxTokens?: number; maxUsd?: number };
  readonly maxHops?: number;  // Default 8, hard ceiling 64
  readonly hopCount?: number;  // How many hops already traveled (0 on originator)
  readonly spent?: { tokens?: number; usd?: number };  // Caller-reported usage
}

export interface Budget {
  maxHops: number;
  maxTokens: number | null;  // null = unbounded
  maxUsd: number | null;  // null = unbounded
}

export interface BudgetEnforcement {
  ok: boolean;
  reason: string;  // Constant string to prevent oracle leaks
  nextHopCount: number;
}

// Validation (synchronous, atomic)
function validateBudget(
  budget: Budget | undefined,
  maxHops: number | undefined
): { ok: boolean; error: string; budget: Budget }

// Enforcement (synchronous, atomic)
function enforceBudget(
  budget: Budget,
  hopCount: number,
  spent: { tokens: number; usd: number }
): BudgetEnforcement
```

**Budget Enforcement Rules:**

| Field | Default when omitted | Notes |
|---|---|---|
| `maxHops` | `8` | `0` disallows remote delegation entirely. Hard ceiling 64. |
| `maxTokens` | unbounded | Σ tokens across the whole hop chain. Hard ceiling 1B. |
| `maxUsd` | unbounded | Σ USD across hops. Hard ceiling $1M. |
| `hopCount` | `0` | Pass-through for messages being re-forwarded. |
| `spent.{tokens,usd}` | `0` | Caller-reported usage from previous legs. Negatives clamped to 0. |

Validation rejects `NaN`, ±`Infinity`, negative numbers, and non-integer hop counts up front. Errors surface as constant strings with no remaining-budget echo, so a malicious caller cannot use response codes as an oracle to probe configured thresholds.

#### Federation Coordinator

```typescript
export class FederationCoordinator {
  private readonly config: FederationCoordinatorConfig;
  private readonly discovery: DiscoveryService;
  private readonly handshake: HandshakeService;
  private readonly routing: RoutingService;
  private readonly audit: AuditService;
  private readonly piiPipeline: PIIPipelineService;
  private readonly trustEvaluator: TrustEvaluator;
  private readonly policyEngine: PolicyEngine;
  private readonly sessions: Map<string, FederationSession>;

  async initialize(manifest: Omit<FederationManifest, 'signature'>): Promise<void>;
  async shutdown(): Promise<void>;
  async joinPeer(endpoint: string): Promise<FederationSession>;
  async leavePeer(nodeId: string): Promise<void>;

  async sendMessage<T>(
    targetNodeId: string,
    messageType: FederationMessageType,
    payload: T,
    options: SendOptions = {}
  ): Promise<RoutingResult>;

  async broadcastMessage<T>(
    messageType: FederationMessageType,
    payload: T
  ): Promise<RoutingResult[]>;

  handleThreatDetection(nodeId: string): void;
  handleHmacFailure(nodeId: string): void;

  getStatus(): FederationStatus;
  getSession(sessionId: string): FederationSession | undefined;
  getActiveSessions(): FederationSession[];
}
```

#### PII Pipeline

14-type PII detection with per-trust-level policies:

```typescript
interface PIIDetectionResult {
  detected: boolean;
  types: PIIType[];
  confidence: number;
  policy: PIIPolicy;
}

enum PIIPolicy {
  BLOCK,    // Reject the message entirely
  REDACT,   // Remove PII before transmission
  HASH,     // Hash PII before transmission
  PASS,     // Allow PII through
}

enum PIIType {
  EMAIL,
  PHONE,
  SSN,
  CREDIT_CARD,
  BANK_ACCOUNT,
  PASSPORT,
  DRIVERS_LICENSE,
  IP_ADDRESS,
  MAC_ADDRESS,
  MEDICAL_RECORD,
  FINANCIAL_RECORD,
  BIOOMETRIC,
  GENDER,
  AGE,
}
```

#### Federation Session

```typescript
interface FederationSession {
  sessionId: string;
  localNodeId: string;
  remoteNodeId: string;
  trustLevel: TrustLevel;
  createdAt: number;
  expiresAt: number;
  active: boolean;
  lastActivity: number;
}
```

#### Automatic Downgrade Triggers

Immediately downgrade a peer to UNTRUSTED when:
- 2+ threat detections in 1 hour
- Any HMAC verification failure
- Session hijack attempt detected

#### Audit Service

Compliance-grade structured logging:

```typescript
interface AuditEvent {
  eventId: string;
  timestamp: number;
  eventType: string;
  sourceNodeId: string;
  targetNodeId?: string;
  sessionId?: string;
  metadata: Record<string, unknown>;
  compliance: ComplianceMode;
}

enum ComplianceMode {
  HIPAA,
  SOC2,
  GDPR,
  NONE,
}
```

### Coordinator Agent

```markdown
---
name: federation-coordinator
description: Orchestrates cross-installation agent federation with zero-trust security
model: opus
---
You are a federation coordinator agent. Your responsibilities:

1. **Discover** remote federation peers via static config, DNS-SD, or IPFS registry
2. **Authenticate** peers using mTLS + ed25519 challenge-response handshake
3. **Evaluate** trust continuously using the scoring formula: `0.4×success_rate + 0.2×uptime + 0.2×(1-threat_penalty) + 0.2×data_integrity`
4. **Route** messages through the PII pipeline and AI Defence gates before transmission
5. **Audit** every federation event with compliance-grade structured logging
6. **Enforce budgets** (ADR-097 Phase 1): every send carries `maxHops` (default 8), with optional `maxTokens` / `maxUsd` caps. The coordinator validates inputs, decrements hop counts, and refuses sends with constant-string errors (`HOP_LIMIT_EXCEEDED`, `BUDGET_EXCEEDED`, `INVALID_BUDGET`) when limits are exceeded — no oracle leak on the failure response.

### Trust Levels

| Level | Name | Capabilities |
|-------|------|-------------|
| 0 | UNTRUSTED | Discovery only |
| 1 | VERIFIED | Status, ping |
| 2 | ATTESTED | Send/receive tasks, query memory (redacted) |
| 3 | TRUSTED | Share context, collaborative execution |
| 4 | PRIVILEGED | Full memory, remote agent spawning |

### Tools

- `npx -y -p @claude-flow/plugin-agent-federation@latest ruflo-federation init` -- generate keypair, create config
- `npx -y -p @claude-flow/plugin-agent-federation@latest ruflo-federation join <endpoint>` -- connect to peer
- `npx -y -p @claude-flow/plugin-agent-federation@latest ruflo-federation peers` -- list peers with trust levels
- `npx -y -p @claude-flow/plugin-agent-federation@latest ruflo-federation status` -- health dashboard
- `npx -y -p @claude-flow/plugin-agent-federation@latest ruflo-federation audit --compliance hipaa` -- audit logs
- `npx -y -p @claude-flow/plugin-agent-federation@latest ruflo-federation trust <node-id> --review` -- trust breakdown
- `npx -y -p @claude-flow/plugin-agent-federation@latest ruflo-federation send <node-id> <msg-type> <payload> [--max-hops N] [--max-tokens N] [--max-usd N]` -- delegate with budget guardrails

### Automatic Downgrade

Immediately downgrade a peer to UNTRUSTED when:
- 2+ threat detections in 1 hour
- Any HMAC verification failure
- Session hijack attempt detected
```

### AutoDev Inspiration Opportunities

1. **Cross-Machine Fleet Collaboration**: Enable AutoDev fleets across multiple developer PCs with zero-trust federation
2. **Budget Circuit Breakers**: Prevent runaway task delegation with hop limits and cost caps
3. **PII-Gated Data Flow**: Detect and redact sensitive data before cross-machine transmission
4. **Trust-Based Authorization**: Implement trust scoring for remote worker access
5. **Compliance Audit Trails**: Add HIPAA/SOC2/GDPR audit logging for enterprise deployments
6. **Secure Messaging**: Add HMAC-signed envelopes for all inter-fleet communication
7. **Automatic Downgrade**: Automatically revoke trust on security violations

---

## 4. Plugin System (NPM Packages)

### Overview

Ruflo's plugin system provides a modular, extensible architecture for adding capabilities via npm packages. Plugins can register agents, tasks, MCP tools, CLI commands, memory backends, hooks, workers, and LLM providers through a well-defined interface.

### Key Features

- **32 Official Plugins**: Core, memory, intelligence, security, DevOps, domain-specific
- **Extension Points**: 8 types (agents, tasks, MCP tools, CLI commands, memory backends, hooks, workers, providers)
- **Dependency Resolution**: Automatic topological sort with circular dependency detection
- **Lifecycle Management**: Initialize, shutdown, health check with timeout protection
- **Plugin Registry**: Central registry with extension point caching
- **Health Monitoring**: Periodic health checks with detailed diagnostics
- **Namespace Coordination**: AgentDB namespace ownership for data isolation
- **Smoke Tests**: Contract validation via shell scripts

### Code Locations

**Core Plugin Interface:**
- `v3/@claude-flow/plugins/src/core/plugin-interface.ts` (216 lines) - IPlugin interface definition
- `v3/@claude-flow/plugins/src/core/base-plugin.ts` - Base plugin implementation
- `v3/@claude-flow/shared/src/plugin-interface.ts` - Shared interface definitions
- `v3/@claude-flow/shared/src/plugin-loader.ts` - Plugin loading logic
- `v3/@claude-flow/shared/src/plugin-registry.ts` - Shared registry implementation

**Registry Implementation:**
- `v3/@claude-flow/plugins/src/registry/plugin-registry.ts` (605 lines) - Main registry
- `v3/@claude-flow/plugins/src/registry/enhanced-plugin-registry.ts` - Enhanced registry
- `v3/@claude-flow/plugins/__tests__/plugin-registry.test.ts` - Registry tests

**CLI Integration:**
- `v3/@claude-flow/cli/src/commands/plugins.ts` - Plugin CLI commands
- `v3/@claude-flow/cli/src/plugins/` - Plugin loading and management

**Plugin Examples:**
- `v3/@claude-flow/plugins/examples/plugin-creator/` - Plugin scaffolding
- `v3/@claude-flow/plugins/examples/ruvector-plugins/` - RuVector integration example

**Official Plugins:**
- `v3/@claude-flow/shared/src/plugins/official/hive-mind-plugin.ts` - Hive mind plugin
- `v3/@claude-flow/shared/src/plugins/official/maestro-plugin.ts` - Maestro plugin

**Plugin Directory:**
- `plugins/README.md` (141 lines) - Plugin catalog and documentation
- `plugins/ruflo-core/` - Core plugin (MCP server, agents)
- `plugins/ruflo-swarm/` - Swarm coordination
- `plugins/ruflo-federation/` - Federation
- `plugins/ruflo-agentdb/` - AgentDB memory
- `plugins/ruflo-intelligence/` - SONA intelligence
- `plugins/ruflo-aidefence/` - Security/PII
- `plugins/ruflo-ruvector/` - RuVector neural stack
- [28 more plugins]

### Architecture Details

#### Plugin Interface

```typescript
export interface IPlugin {
  /** Plugin metadata (name, version, etc.) */
  readonly metadata: PluginMetadata;

  /** Current lifecycle state */
  readonly state: PluginLifecycleState;

  // =========================================================================
  // Lifecycle Methods
  // =========================================================================

  /**
   * Initialize the plugin with context.
   * Called once when the plugin is loaded.
   */
  initialize(context: PluginContext): Promise<void>;

  /**
   * Shutdown the plugin gracefully.
   * Called when the plugin is being unloaded.
   */
  shutdown(): Promise<void>;

  /**
   * Check plugin health.
   * Called periodically for monitoring.
   */
  healthCheck?(): Promise<HealthCheckResult>;

  // =========================================================================
  // Extension Point Registration
  // =========================================================================

  /**
   * Register agent type definitions.
   * Called during initialization to collect agent types.
   */
  registerAgentTypes?(): AgentTypeDefinition[];

  /**
   * Register task type definitions.
   * Called during initialization to collect task types.
   */
  registerTaskTypes?(): TaskTypeDefinition[];

  /**
   * Register MCP tool definitions.
   * Called during initialization to expose tools to Claude.
   */
  registerMCPTools?(): MCPToolDefinition[];

  /**
   * Register CLI command definitions.
   * Called during initialization to extend the CLI.
   */
  registerCLICommands?(): CLICommandDefinition[];

  /**
   * Register memory backend factories.
   * Called during initialization to add storage options.
   */
  registerMemoryBackends?(): MemoryBackendFactory[];

  /**
   * Register hook definitions.
   * Called during initialization to add lifecycle hooks.
   */
  registerHooks?(): HookDefinition[];

  /**
   * Register worker definitions.
   * Called during initialization to add worker types.
   */
  registerWorkers?(): WorkerDefinition[];

  /**
   * Register LLM provider definitions.
   * Called during initialization to add model providers.
   */
  registerProviders?(): LLMProviderDefinition[];
}
```

#### Plugin Metadata

```typescript
interface PluginMetadata {
  name: string;
  version: string;
  description: string;
  author?: string;
  dependencies?: string[];
  capabilities?: string[];
  compatibility?: {
    coreVersion: string;
    nodeVersion?: string;
  };
}
```

#### Plugin Context

```typescript
interface PluginContext {
  config: PluginConfig;
  eventBus: IEventBus;
  logger: ILogger;
  services: ServiceContainer;
  coreVersion: string;
  dataDir: string;
}

interface PluginConfig {
  enabled: boolean;
  priority: number;
  settings: Record<string, unknown>;
}
```

#### Plugin Registry

```typescript
export class PluginRegistry extends EventEmitter {
  private readonly plugins = new Map<string, PluginEntry>();
  private readonly config: PluginRegistryConfig;
  private readonly logger: ILogger;
  private readonly eventBus: IEventBus;
  private readonly services: ServiceContainer;

  // Extension point caches
  private agentTypesCache: AgentTypeDefinition[] = [];
  private taskTypesCache: TaskTypeDefinition[] = [];
  private mcpToolsCache: MCPToolDefinition[] = [];
  private cliCommandsCache: CLICommandDefinition[] = [];
  private memoryBackendsCache: MemoryBackendFactory[] = [];
  private hooksCache: HookDefinition[] = [];
  private workersCache: WorkerDefinition[] = [];
  private providersCache: LLMProviderDefinition[] = [];

  // Plugin loading
  async register(plugin: IPlugin | PluginFactory, config?: Partial<PluginConfig>): Promise<void>;
  async unregister(name: string): Promise<void>;

  // Lifecycle
  async initialize(): Promise<void>;
  async shutdown(): Promise<void>;

  // Dependency resolution
  private resolveDependencies(): string[];

  // Extension points
  private collectExtensionPoints(plugin: IPlugin): void;
  private invalidateCaches(): void;

  // Getters
  getAgentTypes(): AgentTypeDefinition[];
  getTaskTypes(): TaskTypeDefinition[];
  getMCPTools(): MCPToolDefinition[];
  getCLICommands(): CLICommandDefinition[];
  getMemoryBackends(): MemoryBackendFactory[];
  getHooks(): HookDefinition[];
  getWorkers(): WorkerDefinition[];
  getProviders(): LLMProviderDefinition[];

  // Health check
  async healthCheck(): Promise<Map<string, HealthCheckResult>>;

  // Stats
  getStats(): RegistryStats;
}
```

#### Dependency Resolution

Topological sort with circular dependency detection:

```typescript
private resolveDependencies(): string[] {
  const visited = new Set<string>();
  const visiting = new Set<string>();
  const order: string[] = [];

  const visit = (name: string): void => {
    if (visited.has(name)) return;
    if (visiting.has(name)) {
      throw new Error(`Circular dependency detected: ${name}`);
    }

    const entry = this.plugins.get(name);
    if (!entry) {
      throw new Error(`Missing dependency: ${name}`);
    }

    visiting.add(name);

    const deps = entry.plugin.metadata.dependencies ?? [];
    for (const dep of deps) {
      visit(dep);
    }

    visiting.delete(name);
    visited.add(name);
    order.push(name);
  };

  for (const name of this.plugins.keys()) {
    visit(name);
  }

  return order;
}
```

#### Plugin Events

```typescript
export const PLUGIN_EVENTS = {
  LOADING: 'plugin:loading',
  LOADED: 'plugin:loaded',
  INITIALIZING: 'plugin:initializing',
  INITIALIZED: 'plugin:initialized',
  SHUTTING_DOWN: 'plugin:shutting-down',
  SHUTDOWN: 'plugin:shutdown',
  ERROR: 'plugin:error',
  HEALTH_CHECK: 'plugin:health-check',
} as const;
```

#### Plugin Structure

```
ruflo-<name>/
  .claude-plugin/
    plugin.json    # Plugin manifest
  agents/
    <name>.md       # Agent definitions (frontmatter: name, description, model)
  commands/
    <name>.md       # CLI command mappings
  skills/
    <name>/
      SKILL.md      # Interactive skills (frontmatter: name, description, argument-hint, allowed-tools)
  README.md        # Plugin documentation
```

#### Plugin Catalog

**Core & Coordination:**
| Plugin | Description |
|--------|-------------|
| ruflo-core | MCP server, status, doctor, coder/researcher/reviewer agents |
| ruflo-swarm | Swarm topologies (hierarchical, mesh), Monitor streaming |
| ruflo-autopilot | Autonomous /loop task completion with prediction |
| ruflo-loop-workers | 12 background workers via /loop or CronCreate |
| ruflo-workflows | Workflow templates, parallel execution, branching |

**Memory & Intelligence:**
| Plugin | Description |
|--------|-------------|
| ruflo-agentdb | AgentDB with HNSW vector search (150x-12,500x faster) |
| ruflo-rag-memory | SOTA RAG — hybrid search, Graph RAG, MMR diversity, memory bridge |
| ruflo-rvf | Portable RVF memory format, session persistence |
| ruflo-ruvector | ruvector — FlashAttention-3, Graph RAG, hybrid search, 103 MCP tools, Brain AGI |
| ruflo-knowledge-graph | Entity extraction, relation mapping, pathfinder traversal |
| ruflo-intelligence | SONA neural patterns, trajectory learning, model routing |
| ruflo-daa | Dynamic Agentic Architecture, cognitive patterns |

**Quality & Security:**
| Plugin | Description |
|--------|-------------|
| ruflo-security-audit | CVE scanning, dependency vulnerability checks |
| ruflo-aidefence | Prompt injection detection, PII scanning |
| ruflo-testgen | Test gap detection, TDD London School workflow |
| ruflo-browser | Playwright browser automation and testing |

**Development Tools:**
| Plugin | Description |
|--------|-------------|
| ruflo-jujutsu | Diff analysis, risk scoring, reviewer recommendations |
| ruflo-docs | Doc generation, drift detection, API docs |
| ruflo-ruvllm | Local LLM inference, MicroLoRA, chat formatting |
| ruflo-wasm | WASM agent sandboxing and gallery |
| ruflo-plugin-creator | Scaffold and validate new plugins |
| ruflo-migrations | Database schema migration management |
| ruflo-observability | Structured logging, tracing, metrics correlation |
| ruflo-cost-tracker | Token usage tracking, budget alerts, cost optimization |

**Domain-Specific:**
| Plugin | Description |
|--------|-------------|
| ruflo-goals | GOAP planning, deep research, horizon tracking |
| ruflo-federation | Zero-trust cross-installation agent federation |
| ruflo-iot-cognitum | Cognitum Seed IoT — trust scoring, anomaly detection, fleet management |
| ruflo-neural-trader | neural-trader — 4 agents, LSTM/Transformer, Rust/NAPI backtesting, 112+ MCP tools |
| ruflo-market-data | Market data ingestion, OHLCV vectorization, pattern matching |

#### Recommended Stacks

| Use Case | Plugins |
|----------|---------|
| Feature development | ruflo-core + ruflo-swarm + ruflo-testgen + ruflo-ddd |
| Security audit | ruflo-core + ruflo-security-audit + ruflo-aidefence |
| Architecture work | ruflo-core + ruflo-adr + ruflo-ddd + ruflo-sparc |
| Deep research | ruflo-core + ruflo-goals + ruflo-rag-memory + ruflo-intelligence |
| Vector search | ruflo-core + ruflo-ruvector + ruflo-rag-memory + ruflo-knowledge-graph |
| IoT development | ruflo-core + ruflo-iot-cognitum + ruflo-agentdb |
| Trading systems | ruflo-core + ruflo-neural-trader + ruflo-market-data + ruflo-ruvector |
| Full stack | All 32 plugins |

#### npm Package Integration

Several plugins wrap standalone npm packages:

| Plugin | npm Package | What It Adds |
|--------|------------|-------------|
| ruflo-neural-trader | neural-trader | 112+ MCP tools, Rust/NAPI engine, LSTM/Transformer models |
| ruflo-ruvector | ruvector | 103 MCP tools, FlashAttention-3, Graph RAG, Brain AGI |

```bash
# Install backing packages
npm install neural-trader ruvector

# Add as MCP servers (optional, for direct tool access)
claude mcp add neural-trader -- npx neural-trader mcp start
claude mcp add ruvector -- npx ruvector mcp start
```

#### Namespace Coordination

Plugins own AgentDB namespaces for data isolation:

- **Rule**: Namespace format is `<plugin-stem>-<intent>` (kebab-case)
- **Exception**: When plugin name *is* the intent, namespace can match plugin stem
- **Reserved**: `pattern`, `claude-memories`, `default` must not be shadowed
- **Examples**:
  - `ruflo-swarm` owns `swarm-state` namespace
  - `ruflo-federation` owns `federation` namespace
  - `ruflo-agentdb` owns `agentdb` namespace

### AutoDev Inspiration Opportunities

1. **Modular Skills System**: Convert AutoDev Skills to npm plugins for sharing across teams
2. **Recipe Marketplace**: Publish recipes as plugins with dependency management
3. **Memory Backend Plugins**: Allow custom memory backends (e.g., vector DBs) via plugins
4. **Worker Type Plugins**: Register custom worker types (e.g., security-auditor, performance-optimizer)
5. **LLM Provider Plugins**: Add new model providers without core changes
6. **Hook System Plugins**: Register lifecycle hooks (pre-task, post-task, on-failure)
7. **CLI Extension Plugins**: Add custom CLI commands via plugins
8. **Plugin Marketplace**: Create a centralized repository for sharing AutoDev extensions

---

## 5. Ruflo Feature Deep Dive

This section provides detailed exploration of specific Ruflo features, including their codebase locations, complete explanations, and implementation breakdowns. Features are organized by functional category as requested.

### 5.1 Intelligence Group (ruvector)

#### 5.1.1 hooks_route

**Location:** `v3/@claude-flow/cli/src/mcp-tools/hooks-tools.ts` (lines 1-910)

**Purpose:** Semantic routing for intelligent task distribution using vector embeddings and learned patterns.

**Complete Feature Explanation:**

The `hooks_route` tool implements intelligent task routing by analyzing task descriptions and matching them to optimal execution paths. It uses a hybrid approach combining static pattern matching with learned patterns from historical execution data. The routing decision is persisted for learning and can adapt over time based on post-edit outcomes.

**Implementation Breakdown:**

```typescript
{
  name: 'hooks_route',
  description: 'Route tasks to optimal handlers using semantic analysis and learned patterns',
  category: 'intelligence',
  inputSchema: {
    type: 'object',
    properties: {
      task: { type: 'string', description: 'Task description to route' },
      context: { type: 'object', description: 'Additional context for routing' },
      sessionId: { type: 'string', description: 'Session identifier for tracking' },
    },
    required: ['task'],
  },
  handler: async (input) => {
    // 1. Load neural modules lazily
    const vectorDb = await loadVectorDb();
    const memoryStore = await loadMemoryStore();

    // 2. Generate embedding for task description
    const embedding = await vectorDb.embed(input.task);

    // 3. Check static patterns first (fast path)
    const staticMatch = matchStaticPattern(input.task);
    if (staticMatch) {
      return {
        success: true,
        route: staticMatch.route,
        confidence: staticMatch.confidence,
        method: 'static',
      };
    }

    // 4. Search learned patterns via vector similarity
    const similarPatterns = await memoryStore.findSimilarPatterns(embedding, 5);

    // 5. Aggregate scores and select best route
    const route = selectBestRoute(similarPatterns, input.context);

    // 6. Persist routing outcome for learning
    await memoryStore.recordRouting({
      taskId: generateId(),
      task: input.task,
      route,
      embedding,
      timestamp: Date.now(),
    });

    // 7. Assess command risk
    const risk = await assessCommandRisk(input.task, route);

    return {
      success: true,
      route,
      confidence: route.confidence,
      method: 'learned',
      risk,
      alternatives: similarPatterns.slice(1, 3),
    };
  },
}
```

**Key Components:**

- **Vector Database:** Uses native VectorDb or pure JS fallback for embedding storage and similarity search
- **Static Patterns:** Pre-defined patterns for common tasks (fast path optimization)
- **Learned Patterns:** Historical routing decisions stored as embeddings for pattern matching
- **Risk Assessment:** Evaluates potential risks associated with routed commands
- **Outcome Recording:** Post-edit outcomes are recorded to improve future routing decisions

**Data Flow:**

```
Task Description → Embedding Generation → Static Pattern Check
                                          ↓ (if no match)
                                    Vector Similarity Search
                                          ↓
                                    Route Selection & Scoring
                                          ↓
                                    Risk Assessment
                                          ↓
                                    Persistence for Learning
```

---

#### 5.1.2 hooks_stats

**Location:** `v3/@claude-flow/cli/src/mcp-tools/hooks-tools.ts` (lines 1-910)

**Purpose:** Aggregates and reports intelligence statistics from the routing system.

**Complete Feature Explanation:**

The `hooks_stats` tool provides comprehensive analytics on routing performance, pattern usage, and learning progress. It aggregates metrics from the memory store and provides insights into how well the semantic routing system is performing.

**Implementation Breakdown:**

```typescript
{
  name: 'hooks_stats',
  description: 'Get intelligence statistics including routing performance and pattern usage',
  category: 'intelligence',
  inputSchema: {
    type: 'object',
    properties: {
      timeRange: { type: 'string', enum: ['1h', '24h', '7d', '30d', 'all'], description: 'Time range for stats' },
      breakdown: { type: 'boolean', description: 'Include detailed breakdown by route' },
    },
  },
  handler: async (input) => {
    const memoryStore = await loadMemoryStore();

    // 1. Get routing history
    const history = await memoryStore.getRoutingHistory(input.timeRange || '24h');

    // 2. Aggregate statistics
    const stats = {
      totalRoutings: history.length,
      staticPatternMatches: history.filter(h => h.method === 'static').length,
      learnedPatternMatches: history.filter(h => h.method === 'learned').length,
      avgConfidence: calculateAverage(history.map(h => h.confidence)),
      routeDistribution: calculateDistribution(history.map(h => h.route)),
      riskDistribution: calculateDistribution(history.map(h => h.risk)),
      learningProgress: await memoryStore.getLearningProgress(),
      patternClusters: await memoryStore.getPatternClusterStats(),
    };

    // 3. Add breakdown if requested
    if (input.breakdown) {
      stats.breakdown = history.reduce((acc, h) => {
        acc[h.route] = acc[h.route] || { count: 0, avgConfidence: 0, avgRisk: 0 };
        acc[h.route].count++;
        acc[h.route].avgConfidence += h.confidence;
        acc[h.route].avgRisk += h.risk;
        return acc;
      }, {});
    }

    return {
      success: true,
      stats,
      generatedAt: new Date().toISOString(),
    };
  },
}
```

**Key Metrics:**

- **Routing Volume:** Total number of routing decisions
- **Method Distribution:** Static vs. learned pattern usage
- **Confidence Scores:** Average routing confidence over time
- **Route Distribution:** Which routes are most frequently selected
- **Risk Distribution:** Risk levels across routed tasks
- **Learning Progress:** Pattern cluster growth and quality improvement

---

### 5.2 Dev Tools Group (ruflo)

#### 5.2.1 analyze_diff

**Location:** `v3/@claude-flow/cli/src/mcp-tools/analyze-tools.ts` (lines 1-355)

**Purpose:** Analyze git diffs with risk scoring, classification, and reviewer recommendations.

**Complete Feature Explanation:**

The `analyze_diff` tool performs comprehensive analysis of git diffs to assess risk, classify changes, and suggest appropriate reviewers. It integrates with the ruvector diff-classifier module for detailed analysis and provides actionable insights for code review workflows.

**Implementation Breakdown:**

```typescript
{
  name: 'analyze_diff',
  description: 'Analyze git diff with risk scoring, classification, and reviewer suggestions',
  category: 'dev-tools',
  inputSchema: {
    type: 'object',
    properties: {
      diff: { type: 'string', description: 'Git diff content' },
      files: { type: 'array', items: { type: 'string' }, description: 'Changed files' },
      context: { type: 'object', description: 'Additional context (branch, PR number, etc.)' },
    },
    required: ['diff'],
  },
  handler: async (input) => {
    // 1. Parse diff into structured changes
    const changes = parseDiff(input.diff);

    // 2. Load diff-classifier module
    const classifier = await loadDiffClassifier();

    // 3. Analyze each change
    const analyses = await Promise.all(
      changes.map(async (change) => {
        const risk = await classifier.assessRisk(change);
        const classification = await classifier.classify(change);
        const reviewers = await classifier.suggestReviewers(change, classification);
        return {
          file: change.file,
          risk,
          classification,
          reviewers,
          linesAdded: change.linesAdded,
          linesDeleted: change.linesDeleted,
        };
      })
    );

    // 4. Aggregate overall metrics
    const overallRisk = calculateOverallRisk(analyses.map(a => a.risk));
    const overallClassification = aggregateClassifications(analyses.map(a => a.classification));

    return {
      success: true,
      changes: analyses,
      summary: {
        totalFiles: analyses.length,
        overallRisk,
        overallClassification,
        totalLinesAdded: analyses.reduce((sum, a) => sum + a.linesAdded, 0),
        totalLinesDeleted: analyses.reduce((sum, a) => sum + a.linesDeleted, 0),
        suggestedReviewers: deduplicateReviewers(analyses.flatMap(a => a.reviewers)),
      },
      analyzedAt: new Date().toISOString(),
    };
  },
}
```

**Risk Scoring:**

Risk is assessed on multiple dimensions:
- **Complexity:** Cyclomatic complexity of changes
- **Impact:** Files touched and lines modified
- **Domain:** Critical areas (security, database, API)
- **History:** Previous issues in similar files

**Classification Categories:**

- `refactor` - Code restructuring without behavior change
- `feature` - New functionality
- `bugfix` - Bug resolution
- `documentation` - Doc changes only
- `test` - Test additions/modifications
- `config` - Configuration changes
- `security` - Security-related changes

**Related Tools:**

- `diffRisk` - Quick risk assessment for a diff
- `diffClassify` - Classification only
- `diffReviewers` - Reviewer suggestions only
- `fileRisk` - Risk assessment for specific files
- `diffStats` - Quick statistics (files changed, lines added/deleted)

---

#### 5.2.2 performance_benchmark

**Location:** `v3/@claude-flow/cli/src/mcp-tools/performance-tools.ts` (lines 1-725)

**Purpose:** Run synthetic benchmarks to measure system performance under controlled conditions.

**Complete Feature Explanation:**

The `performance_benchmark` tool executes synthetic workloads to measure system performance metrics including latency, throughput, and resource utilization. It provides both real system metrics and synthetic benchmark results for comprehensive performance analysis.

**Implementation Breakdown:**

```typescript
{
  name: 'performance_benchmark',
  description: 'Run synthetic benchmarks to measure system performance',
  category: 'dev-tools',
  inputSchema: {
    type: 'object',
    properties: {
      workload: { type: 'string', enum: ['cpu', 'memory', 'io', 'network', 'mixed'], description: 'Benchmark workload type' },
      duration: { type: 'number', description: 'Duration in seconds (default: 10)' },
      iterations: { type: 'number', description: 'Number of iterations (default: 100)' },
      target: { type: 'string', description: 'Specific target to benchmark (file, endpoint, etc.)' },
    },
    required: ['workload'],
  },
  handler: async (input) => {
    const workload = input.workload as string;
    const duration = (input.duration as number) || 10;
    const iterations = (input.iterations as number) || 100;

    // 1. Gather baseline system metrics
    const baseline = await gatherSystemMetrics();

    // 2. Execute benchmark based on workload type
    const results = await runBenchmark(workload, {
      duration,
      iterations,
      target: input.target,
    });

    // 3. Gather post-benchmark metrics
    const postBenchmark = await gatherSystemMetrics();

    // 4. Calculate deltas
    const deltas = calculateDeltas(baseline, postBenchmark);

    return {
      success: true,
      workload,
      duration,
      iterations,
      results: {
        latency: results.latency,
        throughput: results.throughput,
        errorRate: results.errorRate,
      },
      systemImpact: deltas,
      baseline,
      postBenchmark,
      completedAt: new Date().toISOString(),
    };
  },
}
```

**Benchmark Workloads:**

- **CPU:** Computational tasks (hashing, sorting, mathematical operations)
- **Memory:** Memory allocation and access patterns
- **I/O:** File read/write operations
- **Network:** HTTP requests, socket operations
- **Mixed:** Combination of all workloads

**Metrics Collected:**

- **Latency:** P50, P95, P99 response times
- **Throughput:** Operations per second
- **Error Rate:** Failed operations percentage
- **Resource Usage:** CPU, memory, disk, network utilization
- **System Impact:** Delta between baseline and post-benchmark

---

#### 5.2.3 performance_bottleneck

**Location:** `v3/@claude-flow/cli/src/mcp-tools/performance-tools.ts` (lines 1-725)

**Purpose:** Detect and analyze performance bottlenecks in the system using real metrics.

**Complete Feature Explanation:**

The `performance_bottleneck` tool analyzes real system metrics to identify performance bottlenecks. It examines CPU, memory, I/O, and network metrics to pinpoint areas causing performance degradation and provides actionable recommendations for optimization.

**Implementation Breakdown:**

```typescript
{
  name: 'performance_bottleneck',
  description: 'Detect and analyze performance bottlenecks using system metrics',
  category: 'dev-tools',
  inputSchema: {
    type: 'object',
    properties: {
      component: { type: 'string', description: 'Specific component to analyze (omit for system-wide)' },
      threshold: { type: 'number', description: 'Bottleneck threshold percentage (default: 80)' },
      history: { type: 'string', enum: ['1m', '5m', '15m', '1h'], description: 'Historical data window' },
    },
  },
  handler: async (input) => {
    const threshold = (input.threshold as number) || 80;
    const history = input.history || '5m';

    // 1. Gather current system metrics
    const currentMetrics = await gatherSystemMetrics();

    // 2. Gather historical metrics for comparison
    const historicalMetrics = await gatherHistoricalMetrics(history);

    // 3. Analyze each resource for bottlenecks
    const bottlenecks = [];
    for (const [resource, value] of Object.entries(currentMetrics)) {
      const historicalAvg = calculateAverage(historicalMetrics[resource]);
      const utilization = (value / historicalAvg) * 100;

      if (utilization > threshold) {
        bottlenecks.push({
          resource,
          currentValue: value,
          historicalAverage: historicalAvg,
          utilization,
          severity: utilization > 95 ? 'critical' : utilization > 90 ? 'high' : 'medium',
          recommendations: getRecommendations(resource, utilization),
        });
      }
    }

    // 4. Sort by severity
    bottlenecks.sort((a, b) => b.utilization - a.utilization);

    return {
      success: true,
      bottlenecks,
      summary: {
        totalBottlenecks: bottlenecks.length,
        criticalCount: bottlenecks.filter(b => b.severity === 'critical').length,
        highCount: bottlenecks.filter(b => b.severity === 'high').length,
        mediumCount: bottlenecks.filter(b => b.severity === 'medium').length,
      },
      analyzedAt: new Date().toISOString(),
    };
  },
}
```

**Bottleneck Detection:**

Bottlenecks are identified when resource utilization exceeds the threshold (default 80%) compared to historical averages. Resources analyzed include:

- **CPU:** Processor utilization
- **Memory:** RAM usage and swap
- **Disk I/O:** Read/write operations and latency
- **Network:** Bandwidth and connection counts
- **Database:** Query performance and connection pool

**Recommendations:**

Each bottleneck includes specific recommendations such as:
- Scale resources horizontally/vertically
- Optimize queries or algorithms
- Implement caching
- Reduce payload sizes
- Load balance traffic

---

#### 5.2.4 github_pr_manage

**Location:** `v3/@claude-flow/cli/src/mcp-tools/github-tools.ts` (lines 1-611)

**Purpose:** Full pull request lifecycle management via GitHub CLI integration.

**Complete Feature Explanation:**

The `github_pr_manage` tool provides comprehensive pull request management capabilities by integrating with the GitHub CLI (`gh`). It supports listing, creating, reviewing, merging, and closing pull requests. When the GitHub CLI is unavailable, it falls back to local JSON state storage for offline operation.

**Implementation Breakdown:**

```typescript
{
  name: 'github_pr_manage',
  description: 'Manage GitHub pull requests (list, create, review, merge, close)',
  category: 'dev-tools',
  inputSchema: {
    type: 'object',
    properties: {
      action: {
        type: 'string',
        enum: ['list', 'create', 'review', 'merge', 'close', 'update'],
        description: 'Action to perform',
      },
      prNumber: { type: 'number', description: 'PR number (for review, merge, close)' },
      title: { type: 'string', description: 'PR title (for create)' },
      body: { type: 'string', description: 'PR body (for create, update)' },
      head: { type: 'string', description: 'Head branch (for create)' },
      base: { type: 'string', description: 'Base branch (for create)' },
      reviewComment: { type: 'string', description: 'Review comment (for review)' },
      reviewState: { type: 'string', enum: ['approve', 'request_changes', 'comment'], description: 'Review state' },
      mergeMethod: { type: 'string', enum: ['merge', 'squash', 'rebase'], description: 'Merge method' },
    },
    required: ['action'],
  },
  handler: async (input) => {
    const action = input.action as string;

    // Try GitHub CLI first
    const ghAvailable = await isGitHubCliAvailable();

    if (action === 'list') {
      if (ghAvailable) {
        const result = await exec('gh', ['pr', 'list', '--json', 'number,title,state,author']);
        return { success: true, prs: JSON.parse(result.stdout) };
      }
      // Fallback to local state
      return { success: true, prs: loadLocalPrState() };
    }

    if (action === 'create') {
      if (!input.title || !input.head) {
        return { success: false, error: 'title and head are required for create' };
      }
      if (ghAvailable) {
        const args = [
          'pr', 'create',
          '--title', input.title,
          '--head', input.head,
          '--base', input.base || 'main',
        ];
        if (input.body) args.push('--body', input.body);
        const result = await exec('gh', args);
        return { success: true, prUrl: result.stdout.trim() };
      }
      // Fallback: record in local state
      const pr = { number: Date.now(), title: input.title, head: input.head, base: input.base || 'main', state: 'open' };
      saveLocalPrState(pr);
      return { success: true, pr, offline: true };
    }

    if (action === 'review') {
      if (!input.prNumber) return { success: false, error: 'prNumber is required for review' };
      if (ghAvailable) {
        const args = ['pr', 'review', input.prNumber.toString()];
        if (input.reviewState) args.push(input.reviewState);
        if (input.reviewComment) args.push('--comment', input.reviewComment);
        await exec('gh', args);
        return { success: true, reviewed: true, prNumber: input.prNumber };
      }
      return { success: false, error: 'Review requires GitHub CLI' };
    }

    if (action === 'merge') {
      if (!input.prNumber) return { success: false, error: 'prNumber is required for merge' };
      if (ghAvailable) {
        const method = input.mergeMethod || 'merge';
        await exec('gh', ['pr', 'merge', input.prNumber.toString(), '--' + method]);
        return { success: true, merged: true, prNumber: input.prNumber, method };
      }
      return { success: false, error: 'Merge requires GitHub CLI' };
    }

    if (action === 'close') {
      if (!input.prNumber) return { success: false, error: 'prNumber is required for close' };
      if (ghAvailable) {
        await exec('gh', ['pr', 'close', input.prNumber.toString()]);
        return { success: true, closed: true, prNumber: input.prNumber };
      }
      // Fallback: update local state
      updateLocalPrState(input.prNumber, { state: 'closed' });
      return { success: true, closed: true, prNumber: input.prNumber, offline: true };
    }

    return { success: false, error: 'Unknown action' };
  },
}
```

**GitHub CLI Integration:**

The tool uses the `gh` CLI for all GitHub operations:
- `gh pr list` - List pull requests with JSON output
- `gh pr create` - Create a new pull request
- `gh pr review` - Submit a review (approve/request changes/comment)
- `gh pr merge` - Merge a pull request (merge/squash/rebase)
- `gh pr close` - Close a pull request without merging

**Fallback Behavior:**

When the GitHub CLI is unavailable:
- **List:** Returns PRs from local JSON state
- **Create:** Records PR in local state with `offline: true` flag
- **Review:** Returns error (requires actual GitHub)
- **Merge:** Returns error (requires actual GitHub)
- **Close:** Updates local state to mark as closed

**Related Tools:**

- `github_repo_analyze` - Repository analysis
- `github_issue_track` - Issue tracking
- `github_workflow` - Workflow management

---

### 5.3 Security & Safety Group (ruflo)

#### 5.3.1 aidefence_has_pii

**Location:** `v3/@claude-flow/cli/src/mcp-tools/security-tools.ts` (lines 1-559)

**Purpose:** Detect Personally Identifiable Information (PII) in text content using the AIDefence package.

**Complete Feature Explanation:**

The `aidefence_has_pii` tool scans text content for 14 types of PII including email addresses, phone numbers, SSNs, credit cards, bank accounts, passports, driver's licenses, IP addresses, MAC addresses, medical records, financial records, biometric data, gender, and age. It uses the @claude-flow/aidefence package for detection and returns detailed findings.

**Implementation Breakdown:**

```typescript
{
  name: 'aidefence_has_pii',
  description: 'Detect PII in text content using AIDefence',
  category: 'security',
  inputSchema: {
    type: 'object',
    properties: {
      content: { type: 'string', description: 'Text content to scan' },
      types: { type: 'array', items: { type: 'string' }, description: 'PII types to detect (default: all)' },
      context: { type: 'object', description: 'Additional context for detection' },
    },
    required: ['content'],
  },
  handler: async (input) => {
    // 1. Load AIDefence module (lazy loading)
    const aidefence = await loadAIDefence();

    // 2. Configure detection based on input
    const config = {
      types: input.types || [
        'EMAIL', 'PHONE', 'SSN', 'CREDIT_CARD', 'BANK_ACCOUNT',
        'PASSPORT', 'DRIVERS_LICENSE', 'IP_ADDRESS', 'MAC_ADDRESS',
        'MEDICAL_RECORD', 'FINANCIAL_RECORD', 'BIOMETRIC', 'GENDER', 'AGE',
      ],
      confidenceThreshold: 0.7,
    };

    // 3. Scan content for PII
    const scanResult = await aidefence.scanPII(input.content, config);

    // 4. Format results
    const findings = scanResult.matches.map(match => ({
      type: match.type,
      value: match.value,
      confidence: match.confidence,
      position: { start: match.start, end: match.end },
      context: input.content.substring(Math.max(0, match.start - 20), match.end + 20),
    }));

    return {
      success: true,
      hasPII: findings.length > 0,
      findings,
      summary: {
        totalFindings: findings.length,
        byType: groupByType(findings),
        avgConfidence: calculateAverage(findings.map(f => f.confidence)),
      },
      scannedAt: new Date().toISOString(),
    };
  },
}
```

**PII Types Detected:**

| Type | Description | Pattern |
|------|-------------|---------|
| EMAIL | Email addresses | RFC 5322 regex |
| PHONE | Phone numbers | International format |
| SSN | Social Security Numbers | XXX-XX-XXXX format |
| CREDIT_CARD | Credit card numbers | Luhn validation |
| BANK_ACCOUNT | Bank account numbers | IBAN and local formats |
| PASSPORT | Passport numbers | Country-specific formats |
| DRIVERS_LICENSE | Driver's license numbers | State-specific formats |
| IP_ADDRESS | IP addresses | IPv4 and IPv6 |
| MAC_ADDRESS | MAC addresses | XX:XX:XX:XX:XX:XX |
| MEDICAL_RECORD | Medical record numbers | Healthcare formats |
| FINANCIAL_RECORD | Financial identifiers | Account numbers, IDs |
| BIOMETRIC | Biometric data references | Fingerprints, face ID |
| GENDER | Gender information | Text patterns |
| AGE | Age/DOB information | Date patterns, age numbers |

**Detection Strategy:**

- **Pattern Matching:** Regular expressions for structured data (SSN, credit cards, emails)
- **Context Analysis:** NLP for unstructured PII (medical records, financial data)
- **Confidence Scoring:** Each match has a confidence score (0-1)
- **Position Tracking:** Exact character positions for redaction

---

#### 5.3.2 aidefence_scan

**Location:** `v3/@claude-flow/cli/src/mcp-tools/security-tools.ts` (lines 1-559)

**Purpose:** Comprehensive security scan for prompt injection, PII, and AI manipulation threats.

**Complete Feature Explanation:**

The `aidefence_scan` tool performs a multi-faceted security scan using the AIDefence package. It detects prompt injection attacks, PII leakage, and AI manipulation threats. The tool provides detailed analysis with confidence scores and remediation recommendations.

**Implementation Breakdown:**

```typescript
{
  name: 'aidefence_scan',
  description: 'Scan content for security threats (prompt injection, PII, AI manipulation)',
  category: 'security',
  inputSchema: {
    type: 'object',
    properties: {
      content: { type: 'string', description: 'Content to scan' },
      scanTypes: { type: 'array', items: { type: 'string' }, enum: ['prompt_injection', 'pii', 'ai_manipulation', 'all'], description: 'Scan types' },
      severity: { type: 'string', enum: ['low', 'medium', 'high', 'critical'], description: 'Minimum severity to report' },
    },
    required: ['content'],
  },
  handler: async (input) => {
    // 1. Load AIDefence module (lazy loading)
    const aidefence = await loadAIDefence();

    // 2. Configure scan
    const scanTypes = input.scanTypes || ['all'];
    const severity = input.severity || 'medium';

    // 3. Execute scans based on types
    const results = {
      promptInjection: scanTypes.includes('all') || scanTypes.includes('prompt_injection')
        ? await aidefence.scanPromptInjection(input.content)
        : null,
      pii: scanTypes.includes('all') || scanTypes.includes('pii')
        ? await aidefence.scanPII(input.content)
        : null,
      aiManipulation: scanTypes.includes('all') || scanTypes.includes('ai_manipulation')
        ? await aidefence.scanAIManipulation(input.content)
        : null,
    };

    // 4. Filter by severity
    const filtered = filterBySeverity(results, severity);

    // 5. Aggregate findings
    const findings = aggregateFindings(filtered);

    // 6. Generate recommendations
    const recommendations = generateRecommendations(findings);

    return {
      success: true,
      safe: findings.length === 0,
      findings,
      summary: {
        totalFindings: findings.length,
        byType: groupByType(findings),
        bySeverity: groupBySeverity(findings),
      },
      recommendations,
      scannedAt: new Date().toISOString(),
    };
  },
}
```

**Scan Types:**

**Prompt Injection:**
- Detects jailbreak attempts
- Identifies role-playing attacks
- Flags system prompt extraction
- Detects instruction override attempts

**AI Manipulation:**
- Detects reward hacking patterns
- Identifies adversarial examples
- Flags model distillation attempts
- Detects prompt leaking

**PII:**
- All 14 PII types from `aidefence_has_pii`

**Severity Levels:**

| Severity | Description | Action |
|----------|-------------|--------|
| Critical | Immediate security threat | Block content |
| High | Serious security concern | Review and redact |
| Medium | Potential security issue | Flag for review |
| Low | Minor concern | Log for monitoring |

**Related Tools:**

- `aidefence_analyze` - Detailed analysis of specific threats
- `aidefence_stats` - Statistics on security scans
- `aidefence_learn` - Learn from false positives/negatives
- `aidefence_is_safe` - Quick safety check (boolean)

---

### 5.4 Agents & Orchestration Group (ruflo)

#### 5.4.1 swarm_init

**Location:** `v3/@claude-flow/cli/src/mcp-tools/swarm-tools.ts` (lines 1-437)

**Purpose:** Initialize a swarm with persistent state tracking and orphan reconciliation.

**Complete Feature Explanation:**

The `swarm_init` tool creates a new swarm instance with specified topology, agent limits, and strategy. It maintains persistent state in `.claude-flow/swarm/swarm-state.json` and includes orphan reconciliation to detect and clean up swarms from crashed processes. The tool records the host process PID to enable automatic cleanup on process exit.

**Implementation Breakdown:**

```typescript
{
  name: 'swarm_init',
  description: 'Initialize a swarm with persistent state tracking',
  category: 'swarm',
  inputSchema: {
    type: 'object',
    properties: {
      topology: {
        type: 'string',
        enum: ['hierarchical', 'mesh', 'hierarchical-mesh', 'ring', 'star', 'hybrid', 'adaptive'],
        description: 'Swarm topology type',
      },
      maxAgents: { type: 'number', description: 'Maximum number of agents (1-50)' },
      strategy: { type: 'string', description: 'Agent strategy (specialized, balanced, adaptive)' },
      config: { type: 'object', description: 'Additional swarm configuration' },
    },
  },
  handler: async (input) => {
    // 1. Validate input
    if (input.topology && !VALID_TOPOLOGIES.has(input.topology)) {
      return { success: false, error: `Invalid topology: ${input.topology}` };
    }

    const topology = input.topology || 'hierarchical-mesh';
    const maxAgents = Math.min(Math.max(input.maxAgents || 15, 1), 50);
    const strategy = input.strategy || 'specialized';

    // 2. Generate swarm ID
    const swarmId = `swarm-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    const now = new Date().toISOString();

    // 3. Create swarm state
    const swarmState: SwarmState = {
      swarmId,
      topology,
      maxAgents,
      status: 'running',
      agents: [],
      tasks: [],
      config: {
        topology,
        maxAgents,
        strategy,
        communicationProtocol: input.config?.communicationProtocol || 'message-bus',
        autoScaling: input.config?.autoScaling ?? true,
        consensusMechanism: input.config?.consensusMechanism || 'majority',
      },
      createdAt: now,
      updatedAt: now,
      // Record host PID for orphan detection
      pid: process.pid,
    };

    // 4. Persist to store
    const store = loadSwarmStore();
    store.swarms[swarmId] = swarmState;
    saveSwarmStore(store);

    return {
      success: true,
      swarmId,
      topology,
      strategy,
      maxAgents,
      initializedAt: now,
      config: swarmState.config,
      persisted: true,
    };
  },
}
```

**Orphan Reconciliation:**

The `loadSwarmStore()` function includes orphan detection:

```typescript
function reconcileOrphanSwarms(store: SwarmStore): number {
  let reconciled = 0;
  const nowMs = Date.now();

  for (const swarm of Object.values(store.swarms)) {
    if (swarm.status !== 'running') continue;

    let orphanReason: string | null = null;

    // PID-based detection
    if (typeof swarm.pid === 'number') {
      if (!isPidAlive(swarm.pid)) {
        orphanReason = `host process ${swarm.pid} exited`;
      }
    }
    // TTL fallback for pre-PID entries
    else {
      const ageMs = nowMs - new Date(swarm.updatedAt).getTime();
      if (ageMs > 24 * 60 * 60 * 1000) {
        orphanReason = `no pid recorded and heartbeat is 24h stale`;
      }
    }

    if (orphanReason) {
      swarm.status = 'terminated';
      swarm.terminationReason = orphanReason;
      swarm.updatedAt = new Date().toISOString();
      reconciled++;
    }
  }

  return reconciled;
}
```

**Topology Options:**

- **hierarchical:** Coordinator-led hierarchy (default for coding swarms)
- **mesh:** Full peer-to-peer connectivity
- **hierarchical-mesh:** Hybrid with coordinator + peer communication
- **ring:** Circular peer communication
- **star:** Hub-and-spoke topology
- **hybrid:** Mixed topology
- **adaptive:** Auto-scaling topology

**Strategy Options:**

- **specialized:** Clear roles, no overlap (recommended for anti-drift)
- **balanced:** General-purpose coordination
- **adaptive:** Dynamic role assignment

**Related Tools:**

- `swarm_status` - Get swarm status
- `swarm_shutdown` - Shutdown a swarm
- `swarm_health` - Check swarm health

---

#### 5.4.2 coordination_orchestrate

**Location:** `v3/@claude-flow/cli/src/mcp-tools/coordination-tools.ts` (lines 1-827)

**Purpose:** Orchestrate multi-agent coordination with local state tracking (V2 compatibility layer).

**Complete Feature Explanation:**

The `coordination_orchestrate` tool schedules multi-agent orchestration requests with local state persistence. It is a V2 compatibility layer that records orchestration plans but does not execute them directly. For actual execution, callers should use `agent_spawn` + the Task tool, or `hive-mind_spawn` for queen-led coordination. The tool persists orchestration records (last 100) in `.claude-flow/coordination/store.json`.

**Implementation Breakdown:**

```typescript
{
  name: 'coordination_orchestrate',
  description: 'Orchestrate multi-agent coordination (records orchestration, does not execute)',
  category: 'coordination',
  inputSchema: {
    type: 'object',
    properties: {
      task: { type: 'string', description: 'Task to orchestrate' },
      agents: { type: 'array', items: { type: 'string' }, description: 'Agent IDs to coordinate' },
      strategy: { type: 'string', enum: ['parallel', 'sequential', 'pipeline', 'broadcast'], description: 'Orchestration strategy' },
      timeout: { type: 'number', description: 'Timeout in ms' },
    },
    required: ['task'],
  },
  handler: async (input) => {
    const store = loadCoordStore();
    const task = input.task as string;
    const agents = input.agents || Object.keys(store.nodes);
    const strategy = input.strategy || 'parallel';

    const orchestrationId = `orch-${Date.now()}`;

    // Create orchestration record (does NOT execute)
    const orchestration = {
      id: orchestrationId,
      task,
      strategy,
      agents,
      status: 'scheduled' as const,
      scheduledAt: new Date().toISOString(),
      topology: store.topology.type,
    };

    // Persist orchestration (keep last 100)
    type CoordStoreShape = ReturnType<typeof loadCoordStore> & {
      orchestrations?: Array<typeof orchestration>;
    };
    const orchStore = store as CoordStoreShape;
    if (!Array.isArray(orchStore.orchestrations)) orchStore.orchestrations = [];
    orchStore.orchestrations.push(orchestration);
    if (orchStore.orchestrations.length > 100) {
      orchStore.orchestrations = orchStore.orchestrations.slice(-100);
    }
    saveCoordStore(orchStore);

    return {
      success: true,
      orchestrationId,
      task,
      strategy,
      agents,
      status: 'scheduled',
      topology: store.topology.type,
      executor: 'none',  // No executor wired up
      _note: 'coordination_orchestrate currently records the orchestration request but does not execute it. For real multi-agent execution use agent_spawn + the Task tool, or hive-mind_spawn for queen-led coordination.',
    };
  },
}
```

**Important Note (ADR-093 F7):**

This tool is a V2 compatibility layer that provides **local state management only**:
- Topology/consensus state is tracked locally
- No actual distributed coordination
- Useful for single-machine workflow orchestration

For actual multi-agent execution:
- Use `agent_spawn` + the Task tool
- Use `hive-mind_spawn` for queen-led coordination

**Orchestration Strategies:**

- **parallel:** All agents execute simultaneously
- **sequential:** Agents execute in order
- **pipeline:** Output of one agent feeds into the next
- **broadcast:** Same task sent to all agents

**State Persistence:**

Orchestrations are persisted in `.claude-flow/coordination/store.json`:
```json
{
  "topology": { "type": "hierarchical", "maxNodes": 15, ... },
  "loadBalance": { "algorithm": "adaptive", ... },
  "sync": { "lastSync": "...", ... },
  "nodes": { "node-1": { "status": "active", ... }, ... },
  "orchestrations": [
    { "id": "orch-123", "task": "...", "strategy": "parallel", ... }
  ],
  "version": "3.0.0"
}
```

**Related Tools:**

- `coordination_topology` - Configure swarm topology
- `coordination_load_balance` - Configure load balancing
- `coordination_sync` - Synchronize state across nodes
- `coordination_node` - Manage coordination nodes
- `coordination_consensus` - Manage consensus protocol
- `coordination_metrics` - Get coordination metrics

---

### 5.5 Browser Automation Group (ruflo)

#### 5.5.1 Browser Automation Tools Overview

**Location:** `v3/@claude-flow/cli/src/mcp-tools/browser-tools.ts` (lines 1-705)

**Purpose:** End-to-end browser automation via the @claude-flow/browser package and external CLI tool.

**Complete Feature Explanation:**

The browser automation tools provide comprehensive control over web browsers through an external CLI tool called `agent-browser`. The tools support navigation, element interaction, information retrieval, screenshots, JavaScript evaluation, and session management. This enables end-to-end web testing, scraping, and automation workflows.

**Tool List:**

**Navigation Tools:**
- `browser_open` - Open a URL in the browser
- `browser_back` - Navigate back in history
- `browser_forward` - Navigate forward in history
- `browser_reload` - Reload the current page
- `browser_close` - Close the browser

**Snapshot Tools:**
- `browser_snapshot` - Get a DOM snapshot of the current page
- `browser_screenshot` - Take a screenshot of the current page

**Interaction Tools:**
- `browser_click` - Click an element (CSS selector or text)
- `browser_fill` - Fill a form field
- `browser_type` - Type text into an element
- `browser_press` - Press a keyboard key
- `browser_hover` - Hover over an element
- `browser_select` - Select an option from a dropdown
- `browser_check` - Check a checkbox
- `browser_uncheck` - Uncheck a checkbox
- `browser_scroll` - Scroll the page

**Information Tools:**
- `browser_get_text` - Get text from an element
- `browser_get_value` - Get value from an input field
- `browser_get_title` - Get the page title
- `browser_get_url` - Get the current URL

**Wait Tools:**
- `browser_wait_for_element` - Wait for an element to appear
- `browser_wait_for_text` - Wait for text to appear
- `browser_wait_for_url` - Wait for URL to change
- `browser_sleep` - Sleep for a specified duration

**Execution Tools:**
- `browser_execute` - Execute JavaScript in the page context
- `browser_evaluate` - Evaluate JavaScript and return result

**Session Tools:**
- `browser_session_start` - Start a new browser session
- `browser_session_end` - End the current session

---

#### 5.5.2 browser_open

**Implementation Breakdown:**

```typescript
{
  name: 'browser_open',
  description: 'Open a URL in the browser',
  category: 'browser',
  inputSchema: {
    type: 'object',
    properties: {
      url: { type: 'string', description: 'URL to open' },
      headless: { type: 'boolean', description: 'Run in headless mode (default: false)' },
      viewport: { type: 'object', description: 'Viewport dimensions (width, height)' },
    },
    required: ['url'],
  },
  handler: async (input) => {
    const args = ['open', input.url];
    if (input.headless) args.push('--headless');
    if (input.viewport) {
      args.push('--width', String(input.viewport.width));
      args.push('--height', String(input.viewport.height));
    }

    const result = await execBrowserCli(args);
    return {
      success: result.success,
      sessionId: result.sessionId,
      url: input.url,
      openedAt: new Date().toISOString(),
    };
  },
}
```

---

#### 5.5.3 browser_click

**Implementation Breakdown:**

```typescript
{
  name: 'browser_click',
  description: 'Click an element by CSS selector or text',
  category: 'browser',
  inputSchema: {
    type: 'object',
    properties: {
      selector: { type: 'string', description: 'CSS selector of the element' },
      text: { type: 'string', description: 'Text content of the element' },
      waitForNavigation: { type: 'boolean', description: 'Wait for navigation after click (default: true)' },
    },
  },
  handler: async (input) => {
    if (!input.selector && !input.text) {
      return { success: false, error: 'selector or text is required' };
    }

    const args = ['click'];
    if (input.selector) args.push('--selector', input.selector);
    if (input.text) args.push('--text', input.text);
    if (input.waitForNavigation === false) args.push('--no-wait');

    const result = await execBrowserCli(args);
    return {
      success: result.success,
      clicked: true,
      selector: input.selector,
      text: input.text,
      clickedAt: new Date().toISOString(),
    };
  },
}
```

---

#### 5.5.4 browser_fill

**Implementation Breakdown:**

```typescript
{
  name: 'browser_fill',
  description: 'Fill a form field with value',
  category: 'browser',
  inputSchema: {
    type: 'object',
    properties: {
      selector: { type: 'string', description: 'CSS selector of the input field' },
      value: { type: 'string', description: 'Value to fill' },
      clear: { type: 'boolean', description: 'Clear field before filling (default: true)' },
    },
    required: ['selector', 'value'],
  },
  handler: async (input) => {
    const args = ['fill', '--selector', input.selector, '--value', input.value];
    if (input.clear === false) args.push('--no-clear');

    const result = await execBrowserCli(args);
    return {
      success: result.success,
      filled: true,
      selector: input.selector,
      value: input.value,
      filledAt: new Date().toISOString(),
    };
  },
}
```

---

#### 5.5.5 browser_screenshot

**Implementation Breakdown:**

```typescript
{
  name: 'browser_screenshot',
  description: 'Take a screenshot of the current page',
  category: 'browser',
  inputSchema: {
    type: 'object',
    properties: {
      path: { type: 'string', description: 'Path to save screenshot' },
      fullPage: { type: 'boolean', description: 'Capture full page (default: false)' },
      format: { type: 'string', enum: ['png', 'jpeg', 'webp'], description: 'Image format (default: png)' },
    },
  },
  handler: async (input) => {
    const args = ['screenshot'];
    if (input.path) args.push('--path', input.path);
    if (input.fullPage) args.push('--full-page');
    if (input.format) args.push('--format', input.format);

    const result = await execBrowserCli(args);
    return {
      success: result.success,
      path: result.path,
      fullPage: input.fullPage || false,
      format: input.format || 'png',
      capturedAt: new Date().toISOString(),
    };
  },
}
```

---

#### 5.5.6 External CLI Integration

The browser tools use an external CLI tool called `agent-browser`:

```typescript
async function execBrowserCli(args: string[]): Promise<BrowserResult> {
  const command = 'agent-browser';
  const result = await exec(command, args);

  if (result.code !== 0) {
    throw new Error(`Browser CLI failed: ${result.stderr}`);
  }

  return JSON.parse(result.stdout);
}
```

**CLI Command Structure:**

```bash
agent-browser open <url> [--headless] [--width N] [--height N]
agent-browser click --selector <css> [--text <text>] [--no-wait]
agent-browser fill --selector <css> --value <value> [--no-clear]
agent-browser screenshot [--path <path>] [--full-page] [--format <format>]
agent-browser snapshot
agent-browser execute --script <js>
agent-browser wait-for-element --selector <css> [--timeout N]
```

**Session Management:**

The CLI manages browser sessions automatically:
- First command starts a new session
- Subsequent commands reuse the session
- `browser_session_end` explicitly closes the session

**Error Handling:**

All browser tools include:
- Input validation (required fields)
- Selector/text validation
- Timeout handling for wait operations
- Graceful degradation on CLI unavailability

---

### 5.6 Web Search Configuration & Capability

**Location:** Configuration in `config.toml`, capability in agent definitions, metrics in `ruflo/src/ruvocal/src/lib/server/metrics.ts`

**Purpose:** Web search is configured as a system capability that enables agents to access the internet for research and information gathering.

**Complete Feature Explanation:**

Unlike the other feature groups documented in this section, Ruflo's web search functionality is not implemented as a dedicated set of MCP tools. Instead, it is a **configuration-based capability** that controls whether agents have internet access for research purposes. Web search is configured via the `web_search` setting in `config.toml` and exposed as a capability (`web-search`) that can be assigned to agent types, particularly researcher agents.

**Configuration Breakdown:**

```toml
# config.toml
# Web search enables internet access for research
# - disabled: No web access
# - cached: Use cached results when available
# - live: Always fetch fresh results
web_search = "cached"
```

**Type Definition:**

```typescript
// v3/@claude-flow/codex/src/types.ts
export type WebSearchMode = 'disabled' | 'cached' | 'live';
```

**Configuration Validation:**

```typescript
// v3/@claude-flow/codex/src/validators/index.ts
const VALID_WEB_SEARCH_MODES = ['disabled', 'cached', 'live'];

// Validate web_search value
const webSearchMatch = content.match(/web_search\s*=\s*"([^"]+)"/);
if (webSearchMatch) {
  const mode = webSearchMatch[1];
  if (!VALID_WEB_SEARCH_MODES.includes(mode)) {
    errors.push({
      path: 'config.toml',
      message: `Invalid web_search: "${mode}". Valid values: ${VALID_WEB_SEARCH_MODES.join(', ')}`,
      line: findFieldLine(lines, 'web_search'),
    });
  }
}
```

**Agent Capability Assignment:**

```typescript
// v3/@claude-flow/cli/src/commands/agent.ts
const AGENT_TYPES = [
  { value: 'coder', label: 'Coder', hint: 'Code development with neural patterns' },
  { value: 'researcher', label: 'Researcher', hint: 'Research with web access and data analysis' },
  // ...
];

function getAgentCapabilities(type: string): string[] {
  const capabilities: Record<string, string[]> = {
    coder: ['code-generation', 'refactoring', 'debugging', 'testing'],
    researcher: ['web-search', 'data-analysis', 'summarization', 'citation'],
    // ...
  };
  return capabilities[type] || [];
}
```

**Worker Definition:**

```typescript
// v3/@claude-flow/plugins/src/workers/index.ts
static createResearcher(name: string, capabilities?: string[]): WorkerDefinition {
  return {
    type: 'researcher',
    name,
    description: 'Information gathering and analysis worker',
    capabilities: capabilities ?? ['web-search', 'documentation-analysis', 'pattern-recognition'],
    maxConcurrentTasks: 6,
    timeout: 60000,
    priority: 40,
  };
}
```

**Known Tools Reference:**

```typescript
// v3/@claude-flow/guidance/src/manifest-validator.ts
const KNOWN_TOOLS = new Set([
  'Read', 'Write', 'Edit', 'MultiEdit', 'Glob', 'Grep',
  'Bash', 'Task', 'TodoWrite', 'NotebookEdit', 'WebFetch', 'WebSearch',
  'mcp_memory', 'mcp_swarm', 'mcp_hooks', 'mcp_agent',
]);
```

**Metrics Tracking:**

```typescript
// ruflo/src/ruvocal/src/lib/server/metrics.ts
webSearch: {
  requestCount: new Counter({
    name: "web_search_request_count",
    help: "Total number of web search requests",
    registers: [registry],
  }),
  pageFetchCount: new Counter({
    name: "web_search_page_fetch_count",
    help: "Total number of web search page fetches",
    registers: [registry],
  }),
  pageFetchCountError: new Counter({
    name: "web_search_page_fetch_count_error",
    help: "Total number of web search page fetch errors",
    registers: [registry],
  }),
  pageFetchDuration: new Summary({
    name: "web_search_page_fetch_duration_ms",
    help: "Duration of web search page fetches in milliseconds",
    registers: [registry],
    maxAgeSeconds: 5 * 60,
    ageBuckets: 5,
  }),
  embeddingDuration: new Summary({
    name: "web_search_embedding_duration_ms",
    help: "Duration of web search embeddings in milliseconds",
    registers: [registry],
    maxAgeSeconds: 5 * 60,
    ageBuckets: 5,
  }),
}
```

**Configuration Profiles:**

```typescript
// v3/@claude-flow/codex/src/generators/config-toml.ts
const defaultProfiles: Record<string, ConfigProfile> = {
  dev: {
    approvalPolicy: 'never',
    sandboxMode: 'danger-full-access',
    webSearch: 'live',
  },
  safe: {
    approvalPolicy: 'untrusted',
    sandboxMode: 'read-only',
    webSearch: 'disabled',
  },
  ci: {
    approvalPolicy: 'never',
    sandboxMode: 'workspace-write',
    webSearch: 'cached',
  },
};
```

**Key Components:**

- **Configuration File:** `config.toml` with `web_search` setting
- **Validation:** Ensures only valid modes ('disabled', 'cached', 'live') are used
- **Agent Capability:** 'web-search' capability assigned to researcher agents
- **Known Tools:** WebSearch and WebFetch listed in KNOWN_TOOLS set for tool routing
- **Metrics:** Prometheus-style metrics for request count, page fetches, duration, and errors

**Web Search Modes:**

| Mode | Description | Use Case |
|------|-------------|----------|
| **disabled** | No web access allowed | Secure environments, offline mode, CI/CD pipelines |
| **cached** | Use cached results when available | Development, reduced API costs, faster responses |
| **live** | Always fetch fresh results | Research tasks, real-time data, latest information |

**Agent Types with Web Search:**

| Agent Type | Web Search Capability | Description |
|-------------|----------------------|-------------|
| researcher | ✅ Yes | Research with web access and data analysis |
| coder | ❌ No | Code development with neural patterns |
| tester | ❌ No | Comprehensive testing with automation |
| reviewer | ❌ No | Code review with security and quality checks |
| architect | ❌ No | System design with enterprise patterns |

**Intent Routing:**

```typescript
// v3/@claude-flow/plugins/examples/ruvector-plugins/intent-router.ts
{ pattern: 'search find research', category: 'research', 
  handlers: [{ type: 'agent', name: 'researcher', weight: 1.0 }, { type: 'tool', name: 'WebSearch', weight: 0.8 }], 
  priority: 8, examples: ['look up', 'research topic'] }
```

**Important Notes:**

1. **Not MCP Tools:** Web search is not implemented as a set of MCP tools in the v3 codebase. It is a configuration-based capability.

2. **Historical Context:** In archive/v2 code, WebSearch and WebFetch were referenced as tools, but in v3 they are primarily configuration/capability features.

3. **External Integration:** Actual web search functionality may be provided through external MCP servers or integrated services, not through Ruflo's core MCP tools.

4. **Metrics Only:** The metrics in ruvocal track web search operations but do not implement the search functionality itself.

---

### 5.7 Summary of Feature Locations

| Feature Group | Feature | File Location | Lines |
|---------------|---------|---------------|-------|
| Intelligence | hooks_route | `v3/@claude-flow/cli/src/mcp-tools/hooks-tools.ts` | 1-910 |
| Intelligence | hooks_stats | `v3/@claude-flow/cli/src/mcp-tools/hooks-tools.ts` | 1-910 |
| Dev Tools | analyze_diff | `v3/@claude-flow/cli/src/mcp-tools/analyze-tools.ts` | 1-355 |
| Dev Tools | performance_benchmark | `v3/@claude-flow/cli/src/mcp-tools/performance-tools.ts` | 1-725 |
| Dev Tools | performance_bottleneck | `v3/@claude-flow/cli/src/mcp-tools/performance-tools.ts` | 1-725 |
| Dev Tools | github_pr_manage | `v3/@claude-flow/cli/src/mcp-tools/github-tools.ts` | 1-611 |
| Security | aidefence_has_pii | `v3/@claude-flow/cli/src/mcp-tools/security-tools.ts` | 1-559 |
| Security | aidefence_scan | `v3/@claude-flow/cli/src/mcp-tools/security-tools.ts` | 1-559 |
| Orchestration | swarm_init | `v3/@claude-flow/cli/src/mcp-tools/swarm-tools.ts` | 1-437 |
| Orchestration | coordination_orchestrate | `v3/@claude-flow/cli/src/mcp-tools/coordination-tools.ts` | 1-827 |
| Browser | browser_open | `v3/@claude-flow/cli/src/mcp-tools/browser-tools.ts` | 1-705 |
| Browser | browser_click | `v3/@claude-flow/cli/src/mcp-tools/browser-tools.ts` | 1-705 |
| Browser | browser_fill | `v3/@claude-flow/cli/src/mcp-tools/browser-tools.ts` | 1-705 |
| Browser | browser_screenshot | `v3/@claude-flow/cli/src/mcp-tools/browser-tools.ts` | 1-705 |

---

## Summary and Recommendations

### Priority for AutoDev Adoption

**High Priority (Immediate Value):**
1. **Plugin System** - Modular architecture for Skills, Recipes, and extensions
2. **SONA Learning** - Optimize task assignment and recipe selection
3. **Budget Circuit Breakers** - Prevent runaway delegation in worker requests

**Medium Priority (Strategic Value):**
4. **Swarm Topologies** - Beyond hierarchical, add mesh for peer collaboration
5. **Federation System** - Cross-machine fleet collaboration for teams
6. **Consensus Algorithms** - QC approval and conflict resolution

**Lower Priority (Specialized Use Cases):**
7. **Full SONA Neural Stack** - Complete neural learning with LoRA/EWC
8. **Byzantine Consensus** - For high-security deployments
9. **PII Pipeline** - For enterprise compliance

### Implementation Path

**Phase 1: Plugin Foundation**
- Implement IPlugin interface in AutoDev
- Create plugin registry with dependency resolution
- Convert existing Skills to plugin format
- Add plugin loading via npm packages

**Phase 2: Learning Integration**
- Add SONA-like pattern tracking to task execution
- Implement trajectory-based learning from successful tasks
- Add pattern matching for task assignment optimization
- Implement EWC for continual learning

**Phase 3: Advanced Coordination**
- Add mesh topology option for worker collaboration
- Implement consensus-based QC approval
- Add distributed locking for resource contention
- Implement deadlock detection

**Phase 4: Federation**
- Add zero-trust cross-machine federation
- Implement budget circuit breakers
- Add PII detection and policy enforcement
- Implement compliance audit trails

### Code Reference Summary

| Concept | Primary Files | Lines of Code |
|---------|---------------|---------------|
| SONA | `v3/@claude-flow/neural/src/sona-manager.ts` | 907 |
| Swarm Hub | `v3/@claude-flow/swarm/src/coordination/swarm-hub.ts` | 777 |
| Federation Coordinator | `v3/@claude-flow/plugin-agent-federation/src/application/federation-coordinator.ts` | 368 |
| Plugin Registry | `v3/@claude-flow/plugins/src/registry/plugin-registry.ts` | 605 |
| Plugin Interface | `v3/@claude-flow/plugins/src/core/plugin-interface.ts` | 216 |
| Swarm Types | `v3/@claude-flow/shared/src/types/swarm.types.ts` | 256 |

### Additional Resources

- **Ruflo Documentation**: `ruflo-main/docs/USERGUIDE.md` (7,615 lines)
- **ADR Documentation**: `ruflo-main/ruflo/docs/adr/` (16 ADRs)
- **Plugin Catalog**: `ruflo-main/plugins/README.md` (141 lines)
- **SONA Integration Guide**: `v3/@claude-flow/neural/docs/SONA_INTEGRATION.md` (461 lines)

---

**Document Version:** 1.0  
**Last Updated:** 2026-05-08  
**Source:** Ruflo v3.7.0-alpha.11  
**Reference:** `c:\AIRS Foundry\autodev\ruflo-main\`
