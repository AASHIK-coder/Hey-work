# Hey work

Local AI agent that controls your computer. Give it natural language instructions and watch it take screenshots, move your mouse, click, type, and run terminal commands.

Built with Tauri, React, and Rust.

## Demo

Demo media will be published from the Hey work repository releases.

## Modes

**Computer Use Mode** - Takes over your screen. Sees what you see via screenshots and controls your cursor and keyboard directly. Use when the task spans multiple apps or needs full desktop access. You step away while it works.

**Background Mode** - Runs async while you do other things. Uses Chrome DevTools Protocol for web automation and terminal for everything else. Doesn't touch your mouse or keyboard. Faster and more reliable for web + terminal tasks.

## Setup

**Requirements:**
- Rust & Cargo
- Node.js & npm
- Anthropic API key

```bash
# install deps
npm install

# add your api key
echo "ANTHROPIC_API_KEY=your-key-here" > .env

# run dev
npm run tauri dev

# or build for production
npm run tauri build
```

On macOS, you'll need to grant accessibility permissions when prompted (System Settings → Privacy & Security → Accessibility).

## Shortcuts

- `⌃⇧C` - push-to-talk → computer use mode
- `⌃⇧B` - push-to-talk → background mode
- `⌘⇧H` - help mode (screenshot + quick prompt)
- `⌘⇧S` - stop agent

## Stack

- **Frontend**: React, TypeScript, Tailwind, Zustand, Framer Motion
- **Backend**: Rust, Tauri 2, Tokio
- **Models**: Haiku, Sonnet, Opus (selectable in UI)

## Contributing

PRs welcome.

## License

[Apache License 2.0](LICENSE)
