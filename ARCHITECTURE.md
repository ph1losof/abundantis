# Abundantis Architecture

## Overview

Abundantis is a high-performance unified environment variable management system designed for monorepo workflows. It provides a flexible, multi-source architecture that aggregates environment variables from files, shell, memory, and remote sources with intelligent resolution and caching.

## Design Principles

### 1. Performance-First Architecture
- **Zero-copy parsing** via `korni` crate for .env file parsing
- **SIMD-accelerated interpolation** via `germi` crate for variable resolution
- **Lock-free concurrent access** using `dashmap` for shared state
- **Cache-friendly data structures** with `hashbrown` for optimized hashing
- **Small string optimization** with `compact_str` to reduce allocations

### 2. Modular Plugin System
- **Trait-based source abstraction** via `EnvSource` and `AsyncEnvSource` traits
- **Dynamic source registration** through `SourceRegistry` for extensibility
- **Priority-based resolution** ensuring higher-priority sources override lower ones
- **Capability flags** for source feature detection (READ, WRITE, WATCH, etc.)

### 3. Monorepo-Aware Design
- **Provider abstraction** for different monorepo tools (Turbo, Nx, Lerna, pnpm, npm, Yarn, Cargo, Custom)
- **Context-aware resolution** that maps file paths to workspace packages
- **Cascading environment support** for hierarchical configurations
- **Package discovery** through provider-specific config file parsing

## Core Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Abundantis                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
│  │  Source     │  │ Resolution  │  │  Workspace  │  │
│  │  Registry   │──│    Engine   │──│   Manager   │  │
│  └─────────────┘  └─────────────┘  └─────────────┘  │
│         │                 │                 │              │
│         ▼                 ▼                 ▼              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
│  │   Event     │  │    Cache    │  │   Config    │  │
│  │    Bus      │  │   System    │  │   Loader    │  │
│  └─────────────┘  └─────────────┘  └─────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Module Architecture

### 1. Source Management (`source/`)

**Purpose**: Provides extensible system for environment variable sources

**Components**:
- **traits.rs**: Core source abstractions
  - `EnvSource`: Synchronous source trait
  - `AsyncEnvSource`: Asynchronous source trait (future-proofing)
  - `SourceFactory`: Factory pattern for dynamic source creation
  - `SourceId`, `SourceType`, `Priority`, `SourceCapabilities`: Source metadata

- **registry.rs**: Centralized source management
  - Dynamic source registration and retrieval
  - Priority-based sorting for conflict resolution
  - Batch loading from all registered sources
  - Source metadata tracking

- **memory.rs**: In-memory source for programmatic usage
  - Thread-safe variable storage using `parking_lot::Mutex`
  - Insertion order preservation using `indexmap::IndexMap`
  - Version tracking for change detection
  - Direct CRUD operations for testing and runtime configuration

- **file.rs**: File-based .env source
  - LRU caching with `lru` crate
  - Automatic invalidation on file changes
  - Efficient parsing with `korni`
  - Path-aware variable origin tracking

- **shell.rs**: Shell environment source
  - Direct `std::env` access
  - Highest priority (100) to override file sources
  - Immutable snapshot on first load

**Architectural Choice**: 
- **Trait-based abstraction** enables runtime source registration
- **Arc-wrapped sources** for zero-copy sharing across threads
- **Lazy loading** with invalidation to minimize I/O

### 2. Configuration (`config/`)

**Purpose**: Hierarchical configuration system

**Components**:
- **types.rs**: Configuration data structures
  - `AbundantisConfig`: Root configuration with Serde support
  - `WorkspaceConfig`: Monorepo detection and settings
  - `ResolutionConfig`: Variable resolution behavior
  - `InterpolationConfig`: Interpolation rules and limits
  - `FileResolutionConfig`: .env file merging strategy
  - `CacheConfig`: Cache tuning parameters

**Architectural Choices**:
- **Serde-based serialization** for TOML/YAML/JSON config files
- **Default trait derivation** for sensible out-of-the-box behavior
- **CompactString** for all string fields to reduce memory overhead
- **Enum-based types** for compile-time validation

### 3. Workspace Management (`workspace/`)

**Purpose**: Monorepo-aware context resolution

**Components**:
- **context.rs**: Workspace and package context definitions
  - `WorkspaceContext`: Root, package, and env file mapping
  - `PackageInfo`: Package metadata (root, name, relative path)
  - Hash-based equality for fast caching

- **manager.rs**: Workspace state and discovery
  - Package discovery via provider registry
  - Path-to-context mapping with cache
  - Cascading configuration support
  - Efficient context lookups using `hashbrown::HashMap`

- **provider/**: Monorepo tool integrations
  - **registry.rs**: Provider registration and detection
  - **cargo.rs**: Cargo workspace detection
  - **turbo.rs**: Turborepo package discovery
  - **nx.rs**: Nx project detection
  - **lerna.rs**: Lerna workspace parsing
  - **pnpm.rs**: pnpm workspace support
  - **npm.rs**: npm/yarn workspace detection
  - **custom.rs**: User-defined roots

**Architectural Choices**:
- **Provider trait** for uniform monorepo tool abstraction
- **Lazy discovery** with caching for startup performance
- **Context caching** using `DashMap` for concurrent access
- **Pattern-based package discovery** using `globset` for flexibility

### 4. Resolution Engine (`resolution/`)

**Purpose**: Intelligent variable resolution with interpolation

**Components**:
- **ResolvedVariable**: Fully resolved variable with metadata
  - Key, raw value, and resolved value
  - Source origin tracking
  - Warning flags for issues
  - Description support

- **DependencyGraph**: Placeholder for future interpolation graph
  - Circular dependency detection
  - Reference graph traversal

- **ResolutionEngine**: Core resolution logic
  - Multi-source aggregation
  - Priority-based conflict resolution
  - Interpolation with depth limiting
  - Async-friendly design

- **ResolutionCache**: Multi-level caching system
  - Hot LRU cache for frequently accessed variables
  - TTL cache for time-based invalidation
  - Thread-safe operations

**Architectural Choices**:
- **SIMD interpolation** via `germi` for performance
- **Graph-based dependency tracking** for circular reference detection
  - Not yet implemented, but architecture is in place
- **Multi-level caching** to reduce resolution overhead
- **Arc-wrapped results** for zero-copy sharing

### 5. Error Handling (`error.rs`)

**Purpose**: Comprehensive error reporting with LSP integration

**Components**:
- **AbundantisError**: Main error type with variants
  - Configuration errors
  - Workspace errors
  - Source errors
  - Resolution errors (circular dependency, max depth, undefined variable)
  - IO and runtime errors

- **SourceError**: Source-specific errors
  - Parse errors with line/column information
  - Remote errors with provider context
  - Timeout and authentication errors

- **Diagnostic**: LSP-compatible diagnostic messages
  - Severity levels (Error, Warning, Info, Hint)
  - Diagnostic codes for categorization
  - File and position tracking

**Architectural Choices**:
- **thiserror** for ergonomic error handling with automatic Display implementation
- **Detailed error context** with file paths and line numbers
- **LSP-ready diagnostics** for IDE integration
- **Error chain preservation** for debugging

### 6. Event System (`events/`)

**Purpose**: Reactive updates and notifications

**Components**:
- **AbundantisEvent**: Event types
  - Source added/removed
  - Variables changed
  - Cache invalidated

- **EventSubscriber**: Trait for event handling
  - Sync-only design (dyn-compatible)
  - Send + Sync bounds for thread safety

- **EventBus**: Event publishing (stubbed)
  - Channel-based async messaging (planned)
  - Subscription management

**Architectural Choices**:
- **Trait-based subscriber pattern** for extensibility
- **Dincompatible design** removed async method to support trait objects
- **Future-proof structure** for async event bus implementation

### 7. Builder Pattern (`core/`)

**Purpose**: Fluent API for Abundantis construction

**Components**:
- **AbundantisBuilder**: Builder implementation
  - Fluent API with method chaining
  - Async/sync variants
  - Configuration validation

**Architectural Choices**:
- **Builder pattern** for complex configuration
  - Not fully implemented yet, but structure is in place
- **Derive Default** for easy construction
- **Option-based setters** for flexible configuration

## Data Flow

### Initialization Flow

```
1. Load Configuration (AbundantisConfig)
   ↓
2. Initialize WorkspaceManager
   - Detect monorepo type
   - Discover packages
   - Build context cache
   ↓
3. Initialize SourceRegistry
   - Register default sources (file, shell, memory)
   - Register custom sources if configured
   ↓
4. Initialize ResolutionEngine
   - Setup dependency graph
   - Initialize cache
   ↓
5. Initialize EventBus
   - Setup subscribers
   ↓
6. Create Abundantis instance
   ↓
7. Load all sources
   - Each source produces a Snapshot
   - Store in SourceRegistry
```

### Resolution Flow

```
Request: get("DATABASE_URL", path)
   ↓
1. WorkspaceManager.context_for_file(path)
   - Lookup in context cache
   - Return WorkspaceContext
   ↓
2. ResolutionEngine.resolve(key, context, registry)
   - Load all source snapshots
   - Aggregate variables by priority
   ↓
3. For each source (highest to lowest priority):
   - Find variable in source
   - If found, apply interpolation
     - Detect circular dependencies
     - Resolve references recursively
     - Apply depth limit (default: 10)
   - Return resolved value
   ↓
4. Return ResolvedVariable with metadata
```

### Caching Strategy

```
Level 1: Memory Source Cache
  - Direct variable access in memory
  - No locking needed for reads
  
Level 2: Source Snapshot Cache
  - Cached parsed variables per source
  - LRU eviction (default: 1000 entries)
  - Invalidated on source changes
  
Level 3: Resolution Cache
  - Cached resolved variables
  - Hot cache for frequently accessed keys
  - TTL-based invalidation (default: 60s)
```

## Performance Optimizations

### 1. Memory Management
- **CompactString**: Inline small strings, heap-allocate large ones
- **IndexMap**: Preserve insertion order while providing O(1) lookup
- **Arc**: Zero-copy sharing of immutable data
- **SmallVec**: Stack allocation for small collections

### 2. Concurrency
- **parking_lot::Mutex**: Faster than std::sync::Mutex
- **RwLock**: Multiple concurrent readers
- **DashMap**: Sharded concurrent HashMap for lock-free reads
- **Arc + Send + Sync**: Thread-safe data sharing

### 3. Caching
- **LRU Cache**: Evict least recently used entries
- **TTL Cache**: Time-based invalidation
- **Lazy Loading**: Load data only when needed
- **Snapshot Semantics**: Immutability for safe caching

### 4. Algorithmic
- **Priority-based sorting**: Single-pass conflict resolution
- **Hash-based lookup**: O(1) key access
- **Early termination**: Stop search on first match
- **SIMD interpolation**: Vectorized string operations

## Type System Design

### Trait Hierarchy

```
EnvSource (synchronous)
  ├── MemorySource
  ├── FileSource
  └── ShellSource

AsyncEnvSource (asynchronous, future-proofing)
  └── (future remote sources)

SourceFactory
  ├── FileSourceFactory
  ├── ShellSourceFactory
  └── MemorySourceFactory

MonorepoProvider
  ├── CargoProvider
  ├── TurboProvider
  ├── NxProvider
  ├── LernaProvider
  ├── PnpmProvider
  ├── NpmProvider
  └── CustomProvider
```

### Type Aliases

```rust
pub type Result<T> = std::result::Result<T, AbundantisError>;
```

**Rationale**: Simplifies error handling throughout the codebase

## Error Handling Strategy

### Error Categories

1. **Configuration Errors**: Invalid or missing configuration
2. **Workspace Errors**: Monorepo detection/discovery failures
3. **Source Errors**: Source-specific failures (IO, parse, auth)
4. **Resolution Errors**: Variable resolution failures (circular refs, max depth)
5. **IO Errors**: Filesystem operations
6. **Runtime Errors**: Tokio runtime and async execution

### Error Propagation

```
SourceError → AbundantisError::Source → Result<T>
```

**Rationale**: Preserves source context while providing unified error type

### Diagnostic Reporting

**Severity Levels**: Error, Warning, Info, Hint
**Diagnostic Codes**: EDFxxx (env file), RESxxx (resolution), WSxxx (workspace)

**Rationale**: IDE-friendly error reporting with actionable messages

## Configuration System

### Default Behavior

```toml
[workspace]
provider = null
roots = []
cascading = false
env_files = [".env", ".env.local", ".env.development", ".env.production"]
ignores = ["node_modules", "target", ".git", "dist", "build"]

[resolution]
precedence = ["Shell", "File"]
type_check = true
cache.enabled = true
cache.max_size = 1000

[files]
mode = "merge"
order = [".env", ".env.local"]

[interpolation]
enabled = true
max_depth = 10
features = ["variables", "commands"]
```

**Rationale**: Sensible defaults that work out of the box for most projects

## Future Extensions

### Planned Features

1. **Async Source Support**
   - Remote secrets managers (Vault, AWS Secrets Manager)
   - Real-time synchronization
   - Webhook-based updates

2. **Full Dependency Graph**
   - Circular dependency detection
   - Reference tracking
   - Visual dependency tree

3. **Enhanced Event Bus**
   - Async channel-based messaging
   - Event filtering and routing
   - WebSocket support for real-time updates

4. **File Watching**
   - Automatic reload on file changes
   - Debouncing for performance
   - Recursive directory watching

### Extension Points

1. **Custom Sources**: Implement `EnvSource` trait
2. **Custom Providers**: Implement `MonorepoProvider` trait
3. **Custom Interpolation**: Plug into `ResolutionEngine`
4. **Custom Caching**: Implement `CacheBackend` trait (future)

## Testing Strategy

### Unit Tests
- Source behavior tests (memory, file, shell)
- Configuration parsing tests
- Error handling tests
- Workspace detection tests

### Integration Tests
- Multi-source resolution tests
- Interpolation correctness tests
- Concurrent access tests
- Performance benchmarks

### Test Organization
```
tests/
├── config_tests.rs       (42 tests)
├── error_tests.rs        (38 tests)
├── integration_tests.rs  (35 tests)
├── memory_source_tests.rs (27 tests)
├── source_traits_tests.rs (32 tests)
└── workspace_tests.rs     (26 tests)

Total: 212 tests
```

## Build System

### Features

- `default`: Enables `file` and `shell` sources
- `file`: File source with korni parsing
- `shell`: Shell environment source
- `async`: Async runtime with tokio
- `watch`: File watching with notify
- `full`: All features enabled

### Feature Flags in Code

```rust
#[cfg(feature = "async")]
#[cfg(feature = "file")]
#[cfg(feature = "shell")]
#[cfg(feature = "watch")]
```

**Rationale**: Compile-time feature selection to minimize binary size

## Dependencies Analysis

### Core Dependencies

- **korni**: Zero-copy .env file parsing
- **germi**: SIMD-accelerated interpolation
- **hashbrown**: Optimized HashMap implementation
- **parking_lot**: Fast mutex/rwlock primitives
- **compact_str**: Small string optimization
- **dashmap**: Concurrent HashMap for lock-free reads
- **bitflags**: Type-safe flag bitmasks

### Optional Dependencies

- **tokio**: Async runtime (feature: async)
- **notify**: File system watching (feature: watch)
- **serde**: Serialization/deserialization

**Rationale**: Minimal core dependencies with feature-gated optional dependencies

## Security Considerations

1. **Secret Redaction**: Diagnostic messages filter sensitive values
2. **Path Validation**: All paths are canonicalized to prevent directory traversal
3. **Error Messages**: Don't leak internal paths or secrets
4. **Source Priority**: Shell environment (highest priority) is trusted

## Performance Characteristics

### Time Complexity

- Variable lookup: **O(1)** - Hash map access
- Source loading: **O(n)** - n = number of sources
- Resolution: **O(d)** - d = interpolation depth (max: 10)
- Context lookup: **O(1)** - Cached with hash map

### Space Complexity

- Memory: **O(v)** - v = total variables across all sources
- Cache: **O(c)** - c = cache size (default: 1000)
- Metadata: **O(s)** - s = number of sources

### Benchmarks (Expected)

- Parse 1000 variables: < 1ms
- Resolve simple variable: < 10μs
- Resolve with interpolation (depth 5): < 100μs
- Concurrent access (100 threads): Lock-free reads

## Summary

Abundantis's architecture prioritizes:

1. **Performance** through zero-copy operations, SIMD, and efficient data structures
2. **Extensibility** through trait-based plugin system
3. **Correctness** through type-safe abstractions and comprehensive error handling
4. **Usability** through sensible defaults and fluent API
5. **Monorepo Support** through provider abstraction and context awareness

The design balances compile-time guarantees with runtime flexibility, enabling high-performance environment variable management in complex monorepo setups.
