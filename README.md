# guideline

WebSequenceDiagrams-compatible sequence diagram renderer built with Rust and WebAssembly.

## Features

- Simple, intuitive syntax compatible with WebSequenceDiagrams
- Fast rendering powered by Rust and WebAssembly
- Works in browsers and Node.js
- No dependencies at runtime

## Installation

### npm

```bash
npm install @guide-inc-org/guideline
```

### CDN

```html
<script type="module">
  import init, { render } from 'https://cdn.jsdelivr.net/npm/@guide-inc-org/guideline/guideline_wasm.js';
  await init();
  const svg = render('Alice->Bob: Hello');
</script>
```

## Usage

### ES Modules

```javascript
import init, { render } from '@guide-inc-org/guideline';

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
<div class="guideline">
Alice->Bob: Hello
Bob-->Alice: Hi there
</div>

<script type="module">
  import init, { render } from '@guide-inc-org/guideline';
  await init();

  document.querySelectorAll('.guideline').forEach(el => {
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
cd guideline-wasm
wasm-pack build --target web --out-dir ../guideline-js/pkg

# Run tests
cargo test
```

## Project Structure

```
guideline/
├── guideline-core/     # Rust parser + SVG renderer
├── guideline-wasm/     # WebAssembly bindings
├── guideline-js/       # npm package
└── examples/           # Demo files
```

## License

MIT
