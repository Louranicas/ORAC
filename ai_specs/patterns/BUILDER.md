# Builder Pattern — ORAC

> Typestate builder for configuration structs. Compile-time enforcement of required fields.

## Pattern

Use typestate generics to ensure required fields are set before `.build()` is callable.

## Example

```rust
use std::marker::PhantomData;

// Typestate markers
struct NeedsPort;
struct NeedsSocket;
struct Ready;

struct OracConfigBuilder<State> {
    port: Option<u16>,
    socket_path: Option<String>,
    keepalive_s: u64,
    max_frame_size: usize,
    _state: PhantomData<State>,
}

impl OracConfigBuilder<NeedsPort> {
    pub fn new() -> Self {
        Self {
            port: None,
            socket_path: None,
            keepalive_s: 30,
            max_frame_size: 65536,
            _state: PhantomData,
        }
    }

    pub fn port(self, port: u16) -> OracConfigBuilder<NeedsSocket> {
        OracConfigBuilder {
            port: Some(port),
            socket_path: self.socket_path,
            keepalive_s: self.keepalive_s,
            max_frame_size: self.max_frame_size,
            _state: PhantomData,
        }
    }
}

impl OracConfigBuilder<NeedsSocket> {
    pub fn socket_path(self, path: impl Into<String>) -> OracConfigBuilder<Ready> {
        OracConfigBuilder {
            port: self.port,
            socket_path: Some(path.into()),
            keepalive_s: self.keepalive_s,
            max_frame_size: self.max_frame_size,
            _state: PhantomData,
        }
    }
}

impl OracConfigBuilder<Ready> {
    pub fn build(self) -> OracConfig {
        OracConfig {
            port: self.port.unwrap(),
            socket_path: self.socket_path.unwrap(),
            keepalive_s: self.keepalive_s,
            max_frame_size: self.max_frame_size,
        }
    }
}

// Optional fields available in all states
impl<S> OracConfigBuilder<S> {
    pub fn keepalive_s(mut self, s: u64) -> Self {
        self.keepalive_s = s;
        self
    }

    pub fn max_frame_size(mut self, size: usize) -> Self {
        self.max_frame_size = size;
        self
    }
}

pub struct OracConfig {
    pub port: u16,
    pub socket_path: String,
    pub keepalive_s: u64,
    pub max_frame_size: usize,
}
```

## Usage

```rust
// Compiles:
let config = OracConfigBuilder::new()
    .port(8133)
    .socket_path("/run/user/1000/pane-vortex-bus.sock")
    .keepalive_s(60)
    .build();

// Does NOT compile — missing .socket_path():
// let config = OracConfigBuilder::new()
//     .port(8133)
//     .build();  // ERROR: no method `build` on OracConfigBuilder<NeedsSocket>
```

## When to Use

- Configuration structs with required + optional fields
- Connection builders (bridge clients)
- Request builders (frame construction)

## When NOT to Use

- Simple structs with all-optional fields (use `Default` + field setters)
- Structs with <3 fields (overhead not justified)
