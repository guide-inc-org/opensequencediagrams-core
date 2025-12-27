# OpenSequenceDiagrams Core

A sequence diagram parser and SVG renderer built with Rust and WebAssembly. An open-source alternative to WebSequenceDiagrams.

## Features

- Simple, intuitive syntax compatible with WebSequenceDiagrams
- Fast rendering powered by Rust and WebAssembly
- Works in browsers and Node.js
- No dependencies at runtime

## Installation

### npm

```bash
npm install @opensequencediagrams/core
```

### CDN

```html
<script type="module">
  import init, { render } from 'https://cdn.jsdelivr.net/npm/@opensequencediagrams/core/osd_wasm.js';
  await init();
  const svg = render('Alice->Bob: Hello');
</script>
```

## Usage

### ES Modules

```javascript
import init, { render } from '@opensequencediagrams/core';

await init();

const svg = render(`
title Authentication Flow

actor User
participant Server

User->Server: Login
Server-->User: Token
`);

document.getElementById('diagram').innerHTML = svg;
```

### Auto-render with class

```html
<div class="sequence-diagram">
Alice->Bob: Hello
Bob-->Alice: Hi there
</div>

<script type="module">
  import init, { render } from '@opensequencediagrams/core';
  await init();

  document.querySelectorAll('.sequence-diagram').forEach(el => {
    el.innerHTML = render(el.textContent);
  });
</script>
```

## Syntax

### Messages

```
Alice->Bob: Synchronous message
Bob-->Alice: Response (dashed)
Alice->>Bob: Open arrow
Bob-->>Alice: Dashed open arrow
```

### Participants

```
participant Alice
actor User
participant "Long Name" as LN
```

### Notes

```
note left of Alice: Left note
note right of Bob: Right note
note over Alice: Over note
note over Alice,Bob: Spanning note
```

### Blocks

```
alt success
    Alice->Bob: OK
else failure
    Alice->Bob: Error
end

opt optional
    Alice->Bob: Maybe
end

loop retry
    Alice->Bob: Try again
end
```

### Activation

```
Alice->+Bob: Request (activate Bob)
Bob->-Alice: Response (deactivate Bob)

activate Alice
Alice->Bob: Message
deactivate Alice
```

### Other

```
title My Diagram
autonumber
destroy Bob
```

## Development

### Prerequisites

- Rust 1.70+
- wasm-pack

### Build

```bash
# Build Rust library
cargo build

# Build WebAssembly
cd osd-wasm
wasm-pack build --target web --out-dir ../osd-js/pkg

# Run tests
cargo test
```

## Project Structure

```
opensequencediagrams-core/
├── osd-core/     # Rust parser + SVG renderer
├── osd-wasm/     # WebAssembly bindings
├── osd-js/       # npm package
└── examples/     # Demo files
```

## License

MIT
