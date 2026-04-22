# hermes-agent

Hermes Agent CLI — multi-provider AI chat with tool execution and platform adapters.

## Install

```bash
cargo build -p hermes-agent
# binary at: target/debug/hermes
```

## Commands

### `hermes chat`

Start an interactive REPL chat session.

```bash
hermes chat                          # defaults to openai/gpt-4o
hermes chat -m openai/gpt-4o         # specify model
hermes chat -s <session-id>          # resume existing session
hermes chat --no-tools               # disable tool execution
```

### `hermes model`

Manage AI models.

```bash
hermes model list                         # list available models
hermes model set openai/gpt-4o           # set default model
hermes model info openai/gpt-4o          # show model details
```

### `hermes session`

Manage conversation sessions stored in SQLite.

```bash
hermes session list                       # list recent sessions
hermes session show <id>                  # show session details
hermes session search <query>              # search messages
hermes session delete <id>                 # delete a session
```

### `hermes tools`

List and manage available tools.

```bash
hermes tools list                          # list all tools
hermes tools enable <name>                 # enable a tool
hermes tools disable <name>                # disable a tool
```

### `hermes skills`

Manage Hermes skills (agent extensions).

```bash
hermes skills list                          # list installed skills
hermes skills search <query>                # search skill market
hermes skills install <source>              # install a skill
hermes skills uninstall <name>             # uninstall a skill
```

### `hermes config`

Read and write configuration.

```bash
hermes config get defaults.model            # read a config key
hermes config set defaults.model openai/gpt-4o  # write a config key
hermes config show                           # display full config
hermes config edit                           # open config in $EDITOR
```

### `hermes gateway`

Run the HTTP gateway for platform webhooks (Telegram, WeCom).

```bash
hermes gateway start              # start server on port 8080
hermes gateway start --port 9000  # custom port
hermes gateway status             # show gateway configuration
hermes gateway setup             # print setup instructions
hermes gateway stop              # print stop instructions
```

The gateway listens for incoming webhooks and routes them to the Agent. See `hermes gateway setup` for configuration steps.

## Configuration

Config file: `~/.config/hermes-agent/config.toml` (XDG compliant)

Key environment variables:

| Variable | Description |
|----------|-------------|
| `HERMES_DEFAULT_MODEL` | Default model (e.g. `openai/gpt-4o`) |
| `HERMES_OPENAI_API_KEY` | OpenAI API key |
| `HERMES_TELEGRAM_BOT_TOKEN` | Telegram bot token |
| `HERMES_TELEGRAM_VERIFY_TOKEN` | Telegram webhook verify token |

Priority: CLI flags > Environment variables > Config file > Defaults
